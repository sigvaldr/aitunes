use anyhow::{anyhow, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};
use reqwest::Client;
use rodio::{Decoder, OutputStream, Sink};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, BufReader, Cursor},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Credentials {
    server_url: String,
    username: String,
    password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JellyfinAuth {
    access_token: String,
    user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JellyfinItem {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Type")]
    item_type: String,
    #[serde(rename = "RunTimeTicks")]
    run_time_ticks: Option<u64>,
    #[serde(rename = "AlbumArtist")]
    album_artist: Option<String>,
    #[serde(rename = "Album")]
    album: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JellyfinItemsResponse {
    items: Vec<JellyfinItem>,
}

struct App {
    credentials: Option<Credentials>,
    auth: Option<JellyfinAuth>,
    songs: Vec<JellyfinItem>,
    list_state: ListState,
    input_mode: InputMode,
    server_url_input: String,
    username_input: String,
    password_input: String,
    error_message: Option<String>,
    current_song: Option<JellyfinItem>,
    sink: Option<Sink>,
    _stream: Option<OutputStream>,
}

#[derive(Debug, Clone, PartialEq)]
enum InputMode {
    ServerUrl,
    Username,
    Password,
    SongList,
}

impl App {
    fn new() -> Self {
        Self {
            credentials: None,
            auth: None,
            songs: Vec::new(),
            list_state: ListState::default(),
            input_mode: InputMode::ServerUrl,
            server_url_input: String::new(),
            username_input: String::new(),
            password_input: String::new(),
            error_message: None,
            current_song: None,
            sink: None,
            _stream: None,
        }
    }

    fn load_credentials(&mut self) -> Result<()> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow!("Could not find config directory"))?
            .join("aitunes");
        
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)?;
        }

        let creds_file = config_dir.join("credentials.json");
        if creds_file.exists() {
            let content = fs::read_to_string(&creds_file)?;
            self.credentials = Some(serde_json::from_str(&content)?);
        }

        Ok(())
    }

    fn save_credentials(&self) -> Result<()> {
        if let Some(ref creds) = self.credentials {
            let config_dir = dirs::config_dir()
                .ok_or_else(|| anyhow!("Could not find config directory"))?
                .join("aitunes");
            
            fs::create_dir_all(&config_dir)?;
            let creds_file = config_dir.join("credentials.json");
            fs::write(&creds_file, serde_json::to_string_pretty(creds)?)?;
        }
        Ok(())
    }

    async fn authenticate(&mut self) -> Result<()> {
        let client = Client::new();
        let server_url = self.server_url_input.trim();
        
        // First, get the public system info to verify server connection
        let system_info_url = format!("{}/System/Info/Public", server_url);
        
        let response = client.get(&system_info_url).send().await?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!("Failed to connect to Jellyfin server. Status: {}, Error: {}", status, error_text));
        }

        // Authenticate using the correct Jellyfin API format
        let auth_url = format!("{}/Users/authenticatebyname", server_url);
        let auth_body = serde_json::json!({
            "Username": self.username_input.trim(),
            "Pw": self.password_input.trim()
        });

        let response = client
            .post(&auth_url)
            .header("Content-Type", "application/json")
            .header("X-Emby-Authorization", "MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Version=\"1.0.0\"")
            .json(&auth_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!("Authentication failed. Status: {}, Error: {}", status, error_text));
        }

        let auth_response: serde_json::Value = response.json().await?;
        
        let access_token = auth_response["AccessToken"]
            .as_str()
            .ok_or_else(|| anyhow!("No access token in response"))?;
        let user_id = auth_response["User"]["Id"]
            .as_str()
            .ok_or_else(|| anyhow!("No user ID in response"))?;

        self.auth = Some(JellyfinAuth {
            access_token: access_token.to_string(),
            user_id: user_id.to_string(),
        });

        // Save credentials
        self.credentials = Some(Credentials {
            server_url: server_url.to_string(),
            username: self.username_input.trim().to_string(),
            password: self.password_input.trim().to_string(),
        });
        self.save_credentials()?;

        Ok(())
    }

    async fn load_songs(&mut self) -> Result<()> {
        let auth = self.auth.as_ref().ok_or_else(|| anyhow!("Not authenticated"))?;
        let client = Client::new();
        
        let songs_url = format!(
            "{}/Users/{}/Items?Recursive=true&IncludeItemTypes=Audio&SortBy=Name&Limit=100",
            self.credentials.as_ref().unwrap().server_url,
            auth.user_id
        );

        let response = client
            .get(&songs_url)
            .header("X-Emby-Authorization", format!("MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Token=\"{}\", Version=\"1.0.0\"", auth.access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!("Failed to load songs: {}", error_text));
        }

        let songs_response: serde_json::Value = response.json().await?;
        
        if let Some(items) = songs_response["Items"].as_array() {
            // Convert JSON items to JellyfinItem structs
            let mut songs = Vec::new();
            for item in items {
                if let Ok(jellyfin_item) = serde_json::from_value::<JellyfinItem>(item.clone()) {
                    songs.push(jellyfin_item);
                } else {
                    // Debug: print what failed to parse
                    eprintln!("Failed to parse item: {:?}", item);
                }
            }
            
            eprintln!("Loaded {} songs", songs.len());
            self.songs = songs;
        } else {
            eprintln!("No Items field found in response");
            self.songs = Vec::new();
        }
        
        if !self.songs.is_empty() {
            self.list_state.select(Some(0));
        }

        Ok(())
    }

    async fn play_song(&mut self, song: &JellyfinItem) -> Result<()> {
        let auth = self.auth.as_ref().ok_or_else(|| anyhow!("Not authenticated"))?;
        let client = Client::new();
        
        // Get the direct URL for the song - use the correct Jellyfin streaming format
        let stream_url = format!(
            "{}/Audio/{}/stream",
            self.credentials.as_ref().unwrap().server_url,
            song.id
        );

        eprintln!("Streaming from URL: {}", stream_url);

        // Download the audio data with proper authorization header
        let response = client
            .get(&stream_url)
            .header("X-Emby-Authorization", format!("MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Token=\"{}\", Version=\"1.0.0\"", auth.access_token))
            .send()
            .await?;
        eprintln!("Stream response status: {}", response.status());
        
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            eprintln!("Stream error: {}", error_text);
            return Err(anyhow!("Failed to stream song: {}", error_text));
        }

        let audio_data = response.bytes().await?;
        eprintln!("Downloaded {} bytes of audio data", audio_data.len());
        
        // Create audio sink
        let (_stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;
        
        // Decode and play the audio
        let cursor = Cursor::new(audio_data.to_vec());
        let source = Decoder::new(BufReader::new(cursor))?;
        sink.append(source);
        
        self.sink = Some(sink);
        self._stream = Some(_stream);
        self.current_song = Some(song.clone());

        eprintln!("Audio playback started successfully");
        Ok(())
    }

    fn stop_current_song(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self._stream = None;
        self.current_song = None;
    }

    fn next_input_mode(&mut self) {
        self.input_mode = match self.input_mode {
            InputMode::ServerUrl => InputMode::Username,
            InputMode::Username => InputMode::Password,
            InputMode::Password => InputMode::SongList,
            InputMode::SongList => InputMode::SongList,
        };
    }

    fn handle_input(&mut self, input: char) {
        match self.input_mode {
            InputMode::ServerUrl => {
                self.server_url_input.push(input);
            }
            InputMode::Username => {
                self.username_input.push(input);
            }
            InputMode::Password => {
                self.password_input.push(input);
            }
            InputMode::SongList => {}
        }
    }

    fn handle_backspace(&mut self) {
        match self.input_mode {
            InputMode::ServerUrl => {
                self.server_url_input.pop();
            }
            InputMode::Username => {
                self.username_input.pop();
            }
            InputMode::Password => {
                self.password_input.pop();
            }
            InputMode::SongList => {}
        }
    }

    fn next_song(&mut self) {
        if !self.songs.is_empty() {
            let i = match self.list_state.selected() {
                Some(i) => {
                    if i >= self.songs.len() - 1 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            self.list_state.select(Some(i));
        }
    }

    fn previous_song(&mut self) {
        if !self.songs.is_empty() {
            let i = match self.list_state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.songs.len() - 1
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            self.list_state.select(Some(i));
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(f.size());

    // Title
    let title = Paragraph::new("🎵 aiTunes - Jellyfin Music Player")
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Main content area
    match app.input_mode {
        InputMode::ServerUrl | InputMode::Username | InputMode::Password => {
            render_login_screen(f, chunks[1], app);
        }
        InputMode::SongList => {
            render_song_list(f, chunks[1], app);
        }
    }

    // Status bar
    let status_text = match app.input_mode {
        InputMode::ServerUrl => "Enter Jellyfin server URL (e.g., http://localhost:8096)".to_string(),
        InputMode::Username => "Enter username".to_string(),
        InputMode::Password => "Enter password".to_string(),
        InputMode::SongList => {
            if let Some(ref song) = app.current_song {
                format!("Now playing: {} - {}", song.name, song.album_artist.as_deref().unwrap_or("Unknown"))
            } else {
                "Select a song and press Enter to play".to_string()
            }
        }
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[2]);
}

fn render_login_screen(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Length(3)].as_ref())
        .split(area);

    // Server URL input
    let server_url_style = if app.input_mode == InputMode::ServerUrl {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };
    let server_url = Paragraph::new(format!("Server URL: {}", app.server_url_input))
        .style(server_url_style)
        .block(Block::default().borders(Borders::ALL).title("Server URL"));
    f.render_widget(server_url, chunks[0]);

    // Username input
    let username_style = if app.input_mode == InputMode::Username {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };
    let username = Paragraph::new(format!("Username: {}", app.username_input))
        .style(username_style)
        .block(Block::default().borders(Borders::ALL).title("Username"));
    f.render_widget(username, chunks[1]);

    // Password input
    let password_style = if app.input_mode == InputMode::Password {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::White)
    };
    let password_display = "*".repeat(app.password_input.len());
    let password = Paragraph::new(format!("Password: {}", password_display))
        .style(password_style)
        .block(Block::default().borders(Borders::ALL).title("Password"));
    f.render_widget(password, chunks[2]);

    // Error message
    if let Some(ref error) = app.error_message {
        let error_area = Rect::new(area.x, area.y + 9, area.width, 3);
        let error_widget = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Error"));
        f.render_widget(Clear, error_area);
        f.render_widget(error_widget, error_area);
    }
}

fn render_song_list(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .songs
        .iter()
        .map(|song| {
            let duration = if let Some(ticks) = song.run_time_ticks {
                let seconds = ticks / 10_000_000;
                let minutes = seconds / 60;
                let remaining_seconds = seconds % 60;
                format!("{:02}:{:02}", minutes, remaining_seconds)
            } else {
                "Unknown".to_string()
            };

            let artist = song.album_artist.as_deref().unwrap_or("Unknown Artist");
            let album = song.album.as_deref().unwrap_or("Unknown Album");
            
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} - {}", song.name, artist),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("[{}]", album),
                    Style::default().fg(Color::Gray),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("({})", duration),
                    Style::default().fg(Color::Blue),
                ),
            ]))
        })
        .collect();

    let songs_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Songs"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");

    f.render_stateful_widget(songs_list, area, &mut app.list_state.clone());
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new();
    app.load_credentials()?;

    // If we have saved credentials, try to authenticate
    if let Some(ref creds) = app.credentials {
        app.server_url_input = creds.server_url.clone();
        app.username_input = creds.username.clone();
        app.password_input = creds.password.clone();
        app.input_mode = InputMode::SongList;
        
        if let Err(e) = app.authenticate().await {
            app.error_message = Some(format!("Authentication failed: {}", e));
            app.input_mode = InputMode::ServerUrl;
            app.server_url_input.clear();
            app.username_input.clear();
            app.password_input.clear();
        } else {
            if let Err(e) = app.load_songs().await {
                app.error_message = Some(format!("Failed to load songs: {}", e));
            }
        }
    }

    // Main loop
    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Esc => {
                        if app.input_mode == InputMode::SongList {
                            break;
                        } else {
                            app.input_mode = InputMode::ServerUrl;
                            app.server_url_input.clear();
                            app.username_input.clear();
                            app.password_input.clear();
                            app.error_message = None;
                        }
                    }
                    KeyCode::Char(c) => {
                        if app.input_mode == InputMode::SongList {
                            match c {
                                'j' | 's' => app.next_song(),
                                'k' | 'w' => app.previous_song(),
                                ' ' => {
                                    if app.sink.is_some() {
                                        app.stop_current_song();
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            app.handle_input(c);
                        }
                    }
                    KeyCode::Backspace => {
                        if app.input_mode != InputMode::SongList {
                            app.handle_backspace();
                        }
                    }
                    KeyCode::Enter => {
                        match app.input_mode {
                            InputMode::ServerUrl => {
                                app.next_input_mode();
                            }
                            InputMode::Username => {
                                app.next_input_mode();
                            }
                            InputMode::Password => {
                                // Try to authenticate
                                app.error_message = None;
                                if let Err(e) = app.authenticate().await {
                                    app.error_message = Some(format!("Authentication failed: {}", e));
                                } else {
                                    app.input_mode = InputMode::SongList;
                                    if let Err(e) = app.load_songs().await {
                                        app.error_message = Some(format!("Failed to load songs: {}", e));
                                    }
                                }
                            }
                            InputMode::SongList => {
                                if let Some(selected) = app.list_state.selected() {
                                    if let Some(song) = app.songs.get(selected) {
                                        let song_name = song.name.clone();
                                        eprintln!("Attempting to play: {}", song_name);
                                        let song_clone = song.clone();
                                        app.stop_current_song();
                                        if let Err(e) = app.play_song(&song_clone).await {
                                            eprintln!("Failed to play song: {}", e);
                                            app.error_message = Some(format!("Failed to play song: {}", e));
                                        } else {
                                            eprintln!("Successfully started playing: {}", song_name);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Up => {
                        if app.input_mode == InputMode::SongList {
                            app.previous_song();
                        }
                    }
                    KeyCode::Down => {
                        if app.input_mode == InputMode::SongList {
                            app.next_song();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}