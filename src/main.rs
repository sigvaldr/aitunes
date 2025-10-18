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

// Theme system
struct Theme {
    primary: Color,    // #00A6D7 - Main foreground color
    background: Color, // #343434 - Background color
    secondary: Color,  // Lighter variant of primary
    accent: Color,     // Complementary color for highlights
    muted: Color,      // Muted version for secondary text
    error: Color,      // Error color
    success: Color,    // Success color
}

impl Theme {
    fn new() -> Self {
        Self {
            primary: Color::Rgb(0, 166, 215),   // #00A6D7
            background: Color::Rgb(52, 52, 52), // #343434
            secondary: Color::Rgb(0, 140, 190), // Darker variant
            accent: Color::Rgb(0, 120, 170),    // Even darker for contrast
            muted: Color::Rgb(150, 150, 150),   // Light gray for secondary text on dark background
            error: Color::Rgb(255, 100, 100),   // Brighter red for errors on dark background
            success: Color::Rgb(100, 200, 100), // Brighter green for success on dark background
        }
    }

    fn primary_style(&self) -> Style {
        Style::default().fg(self.primary).bg(self.background)
    }

    fn secondary_style(&self) -> Style {
        Style::default().fg(self.secondary).bg(self.background)
    }

    fn accent_style(&self) -> Style {
        Style::default().fg(self.accent).bg(self.background)
    }

    fn muted_style(&self) -> Style {
        Style::default().fg(self.muted).bg(self.background)
    }

    fn error_style(&self) -> Style {
        Style::default().fg(self.error).bg(self.background)
    }

    fn success_style(&self) -> Style {
        Style::default().fg(self.success).bg(self.background)
    }

    fn title_style(&self) -> Style {
        Style::default()
            .fg(self.primary)
            .bg(self.background)
            .add_modifier(Modifier::BOLD)
    }

    fn highlight_style(&self) -> Style {
        Style::default()
            .fg(Color::Rgb(0, 0, 0))
            .bg(Color::Rgb(255, 255, 255))
            .add_modifier(Modifier::REVERSED)
    }
}

// Global theme instance
const THEME: Theme = Theme {
    primary: Color::Rgb(0, 166, 215),
    background: Color::Rgb(52, 52, 52),
    secondary: Color::Rgb(0, 140, 190),
    accent: Color::Rgb(0, 120, 170),
    muted: Color::Rgb(150, 150, 150),
    error: Color::Rgb(255, 100, 100),
    success: Color::Rgb(100, 200, 100),
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

#[derive(Debug, Clone)]
enum LibraryItem {
    Artist(String),
    Album(String, String), // (artist, album)
    Song(JellyfinItem),
}

#[derive(Debug, Clone)]
struct LibraryNode {
    item: LibraryItem,
    children: Vec<LibraryNode>,
    expanded: bool,
}

impl LibraryNode {
    fn new(item: LibraryItem) -> Self {
        Self {
            item,
            children: Vec::new(),
            expanded: false,
        }
    }

    fn add_child(&mut self, child: LibraryNode) {
        self.children.push(child);
    }

    fn is_expanded(&self) -> bool {
        self.expanded
    }

    fn toggle_expansion(&mut self) {
        self.expanded = !self.expanded;
    }

    fn get_display_name(&self) -> String {
        match &self.item {
            LibraryItem::Artist(name) => name.clone(),
            LibraryItem::Album(artist, album) => format!("{} - {}", artist, album),
            LibraryItem::Song(song) => song.name.clone(),
        }
    }

    fn get_indent_level(&self) -> usize {
        match &self.item {
            LibraryItem::Artist(_) => 0,
            LibraryItem::Album(_, _) => 1,
            LibraryItem::Song(_) => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JellyfinItemsResponse {
    items: Vec<JellyfinItem>,
}

struct App {
    credentials: Option<Credentials>,
    auth: Option<JellyfinAuth>,
    songs: Vec<JellyfinItem>,
    library_tree: Vec<LibraryNode>,
    flat_library: Vec<LibraryNode>, // Flattened view for navigation
    list_state: ListState,
    input_mode: InputMode,
    server_url_input: String,
    username_input: String,
    password_input: String,
    error_message: Option<String>,
    current_song: Option<JellyfinItem>,
    sink: Option<Sink>,
    _stream: Option<OutputStream>,
    loading_state: LoadingState,
    is_paused: bool,
    queue: Vec<JellyfinItem>,
    queue_state: ListState,
    current_time: u64, // Current playback time in milliseconds
    volume: f32,       // Volume level 0.0 to 1.0
    active_panel: ActivePanel,
    song_start_time: Option<std::time::Instant>, // When the current song started playing
    show_help: bool,                             // Whether to show the help menu
}

#[derive(Debug, Clone)]
enum LoadingState {
    NotLoading,
    LoadingSongs { progress: usize, total: usize },
}

#[derive(Debug, Clone, PartialEq)]
enum ActivePanel {
    Library,
    Queue,
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
            library_tree: Vec::new(),
            flat_library: Vec::new(),
            list_state: ListState::default(),
            input_mode: InputMode::ServerUrl,
            server_url_input: String::new(),
            username_input: String::new(),
            password_input: String::new(),
            error_message: None,
            current_song: None,
            sink: None,
            _stream: None,
            loading_state: LoadingState::NotLoading,
            is_paused: false,
            queue: Vec::new(),
            queue_state: ListState::default(),
            current_time: 0,
            volume: 0.8, // Default to 80% volume
            active_panel: ActivePanel::Library,
            song_start_time: None,
            show_help: false,
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
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!(
                "Failed to connect to Jellyfin server. Status: {}, Error: {}",
                status,
                error_text
            ));
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
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!(
                "Authentication failed. Status: {}, Error: {}",
                status,
                error_text
            ));
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
        let auth = self
            .auth
            .as_ref()
            .ok_or_else(|| anyhow!("Not authenticated"))?;
        let client = Client::new();

        // Start loading state
        self.loading_state = LoadingState::LoadingSongs {
            progress: 0,
            total: 0,
        };

        // First, get the total count
        let count_url = format!(
            "{}/Users/{}/Items?Recursive=true&IncludeItemTypes=Audio&SortBy=Name&Limit=1",
            self.credentials.as_ref().unwrap().server_url,
            auth.user_id
        );

        let response = client
            .get(&count_url)
            .header("X-Emby-Authorization", format!("MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Token=\"{}\", Version=\"1.0.0\"", auth.access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!("Failed to load songs: {}", error_text));
        }

        let count_response: serde_json::Value = response.json().await?;
        let total_count = count_response["TotalRecordCount"].as_u64().unwrap_or(0) as usize;

        self.loading_state = LoadingState::LoadingSongs {
            progress: 0,
            total: total_count,
        };

        // Load all songs in batches
        let mut all_songs = Vec::new();
        let mut start_index = 0;
        let batch_size = 100;

        while start_index < total_count {
            let songs_url = format!(
                "{}/Users/{}/Items?Recursive=true&IncludeItemTypes=Audio&SortBy=Name&StartIndex={}&Limit={}",
                self.credentials.as_ref().unwrap().server_url,
                auth.user_id,
                start_index,
                batch_size
            );

            let response = client
                .get(&songs_url)
                .header("X-Emby-Authorization", format!("MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Token=\"{}\", Version=\"1.0.0\"", auth.access_token))
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                return Err(anyhow!("Failed to load songs: {}", error_text));
            }

            let songs_response: serde_json::Value = response.json().await?;

            if let Some(items) = songs_response["Items"].as_array() {
                for item in items {
                    if let Ok(jellyfin_item) = serde_json::from_value::<JellyfinItem>(item.clone())
                    {
                        all_songs.push(jellyfin_item);
                    }
                }
            }

            start_index += batch_size;
            self.loading_state = LoadingState::LoadingSongs {
                progress: start_index.min(total_count),
                total: total_count,
            };
        }

        self.songs = all_songs;

        // Organize songs into hierarchical structure
        self.organize_library();

        // Stop loading state
        self.loading_state = LoadingState::NotLoading;

        if !self.flat_library.is_empty() {
            self.list_state.select(Some(0));
        }

        Ok(())
    }

    async fn load_songs_with_progress(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        let auth = self
            .auth
            .as_ref()
            .ok_or_else(|| anyhow!("Not authenticated"))?;
        let client = Client::new();

        // Start loading state
        self.loading_state = LoadingState::LoadingSongs {
            progress: 0,
            total: 0,
        };

        // First, get the total count
        let count_url = format!(
            "{}/Users/{}/Items?Recursive=true&IncludeItemTypes=Audio&SortBy=Name&Limit=1",
            self.credentials.as_ref().unwrap().server_url,
            auth.user_id
        );

        let response = client
            .get(&count_url)
            .header("X-Emby-Authorization", format!("MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Token=\"{}\", Version=\"1.0.0\"", auth.access_token))
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!("Failed to load songs: {}", error_text));
        }

        let count_response: serde_json::Value = response.json().await?;
        let total_count = count_response["TotalRecordCount"].as_u64().unwrap_or(0) as usize;

        self.loading_state = LoadingState::LoadingSongs {
            progress: 0,
            total: total_count,
        };

        // Load all songs in batches
        let mut all_songs = Vec::new();
        let mut start_index = 0;
        let batch_size = 100;

        while start_index < total_count {
            // Update UI to show loading progress
            terminal.draw(|f| ui(f, self))?;

            // Small delay to make loading visible
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            let songs_url = format!(
                "{}/Users/{}/Items?Recursive=true&IncludeItemTypes=Audio&SortBy=Name&StartIndex={}&Limit={}",
                self.credentials.as_ref().unwrap().server_url,
                auth.user_id,
                start_index,
                batch_size
            );

            let response = client
                .get(&songs_url)
                .header("X-Emby-Authorization", format!("MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Token=\"{}\", Version=\"1.0.0\"", auth.access_token))
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                return Err(anyhow!("Failed to load songs: {}", error_text));
            }

            let songs_response: serde_json::Value = response.json().await?;

            if let Some(items) = songs_response["Items"].as_array() {
                for item in items {
                    if let Ok(jellyfin_item) = serde_json::from_value::<JellyfinItem>(item.clone())
                    {
                        all_songs.push(jellyfin_item);
                    }
                }
            }

            start_index += batch_size;
            self.loading_state = LoadingState::LoadingSongs {
                progress: start_index.min(total_count),
                total: total_count,
            };
        }

        self.songs = all_songs;

        // Organize songs into hierarchical structure
        self.organize_library();

        // Stop loading state
        self.loading_state = LoadingState::NotLoading;

        if !self.flat_library.is_empty() {
            self.list_state.select(Some(0));
        }

        Ok(())
    }

    fn organize_library(&mut self) {
        use std::collections::HashMap;

        // Group songs by artist and album
        let mut artists: HashMap<String, HashMap<String, Vec<JellyfinItem>>> = HashMap::new();

        for song in &self.songs {
            let artist = song
                .album_artist
                .as_deref()
                .unwrap_or("Unknown Artist")
                .to_string();
            let album = song.album.as_deref().unwrap_or("Unknown Album").to_string();

            artists
                .entry(artist)
                .or_insert_with(HashMap::new)
                .entry(album)
                .or_insert_with(Vec::new)
                .push(song.clone());
        }

        // Build hierarchical structure
        self.library_tree.clear();
        self.flat_library.clear();

        let mut artist_names: Vec<_> = artists.keys().collect();
        artist_names.sort();

        for artist_name in artist_names {
            let mut artist_node = LibraryNode::new(LibraryItem::Artist(artist_name.clone()));

            let mut album_names: Vec<_> = artists[artist_name].keys().collect();
            album_names.sort();

            for album_name in album_names {
                let mut album_node =
                    LibraryNode::new(LibraryItem::Album(artist_name.clone(), album_name.clone()));

                let mut songs = artists[artist_name][album_name].clone();
                songs.sort_by(|a, b| a.name.cmp(&b.name));

                for song in songs {
                    let song_node = LibraryNode::new(LibraryItem::Song(song));
                    album_node.add_child(song_node);
                }

                artist_node.add_child(album_node);
            }

            self.library_tree.push(artist_node);
        }

        // Create flattened view for navigation
        self.flatten_library();
    }

    fn flatten_library(&mut self) {
        self.flat_library.clear();

        for artist_node in &self.library_tree {
            self.flat_library.push(artist_node.clone());

            if artist_node.is_expanded() {
                for album_node in &artist_node.children {
                    self.flat_library.push(album_node.clone());

                    if album_node.is_expanded() {
                        for song_node in &album_node.children {
                            self.flat_library.push(song_node.clone());
                        }
                    }
                }
            }
        }
    }

    async fn play_song(&mut self, song: &JellyfinItem) -> Result<()> {
        let auth = self
            .auth
            .as_ref()
            .ok_or_else(|| anyhow!("Not authenticated"))?;
        let client = Client::new();

        // Try different URL formats for better compatibility
        let urls_to_try = vec![
            format!(
                "{}/Items/{}/Download",
                self.credentials.as_ref().unwrap().server_url,
                song.id
            ),
            format!(
                "{}/Audio/{}/stream",
                self.credentials.as_ref().unwrap().server_url,
                song.id
            ),
            format!(
                "{}/Audio/{}/stream?api_key={}",
                self.credentials.as_ref().unwrap().server_url,
                song.id,
                auth.access_token
            ),
        ];

        let mut audio_data = None;

        for url in urls_to_try {
            let response = client
                .get(&url)
                .header("X-Emby-Authorization", format!("MediaBrowser Client=\"aitunes\", Device=\"Terminal\", DeviceId=\"aitunes-terminal\", Token=\"{}\", Version=\"1.0.0\"", auth.access_token))
                .send()
                .await?;

            if response.status().is_success() {
                match response.bytes().await {
                    Ok(data) => {
                        audio_data = Some(data);
                        break;
                    }
                    Err(_e) => {
                        continue;
                    }
                }
            } else {
                let _error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
            }
        }

        let audio_data = audio_data.ok_or_else(|| anyhow!("All URL formats failed"))?;

        // Create audio sink
        let (_stream, stream_handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&stream_handle)?;

        // Try to decode the audio with better error handling
        let cursor = Cursor::new(audio_data.to_vec());
        let source = match Decoder::new(BufReader::new(cursor)) {
            Ok(source) => source,
            Err(e) => {
                // Try alternative approach: check if it's a streaming format issue
                // Sometimes Jellyfin returns partial data or the wrong content type
                if audio_data.len() == 0 {
                    return Err(anyhow!("No audio data received"));
                }

                // Try to create a new cursor and attempt decoding again
                let cursor2 = Cursor::new(audio_data.to_vec());
                match Decoder::new(BufReader::new(cursor2)) {
                    Ok(source) => source,
                    Err(e2) => {
                        return Err(anyhow!("Unrecognized format: {} (original: {})", e2, e));
                    }
                }
            }
        };

        sink.append(source);
        sink.set_volume(self.volume);

        self.sink = Some(sink);
        self._stream = Some(_stream);
        self.current_song = Some(song.clone());
        self.current_time = 0;
        self.song_start_time = Some(std::time::Instant::now());

        Ok(())
    }

    fn stop_current_song(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self._stream = None;
        self.current_song = None;
        self.is_paused = false;
        self.song_start_time = None;
        self.current_time = 0;
    }

    fn pause_unpause(&mut self) {
        if let Some(ref sink) = self.sink {
            if self.is_paused {
                sink.play();
                self.is_paused = false;
                // Reset start time when resuming
                self.song_start_time = Some(std::time::Instant::now());
            } else {
                sink.pause();
                self.is_paused = true;
                // Update current time when pausing
                self.update_current_time();
            }
        }
    }

    fn update_current_time(&mut self) {
        if let Some(start_time) = self.song_start_time {
            if !self.is_paused {
                let elapsed = start_time.elapsed();
                self.current_time = elapsed.as_millis() as u64;
            }
        }
    }

    fn add_to_queue(&mut self, song: JellyfinItem) {
        self.queue.push(song);
        if self.queue_state.selected().is_none() && !self.queue.is_empty() {
            self.queue_state.select(Some(0));
        }
    }

    fn remove_from_queue(&mut self, index: usize) {
        if index < self.queue.len() {
            self.queue.remove(index);
            if self.queue.is_empty() {
                self.queue_state.select(None);
            } else if let Some(selected) = self.queue_state.selected() {
                if selected >= self.queue.len() {
                    self.queue_state.select(Some(self.queue.len() - 1));
                }
            }
        }
    }

    fn clear_queue(&mut self) {
        self.queue.clear();
        self.queue_state.select(None);
    }

    fn shuffle_queue(&mut self) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::{SystemTime, UNIX_EPOCH};

        if self.queue.len() <= 1 {
            return;
        }

        // Use current time as seed for randomness
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        let mut rng = hasher.finish();

        // Fisher-Yates shuffle
        for i in (1..self.queue.len()).rev() {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let j = (rng as usize) % (i + 1);
            self.queue.swap(i, j);
        }

        // Reset selection to first item
        if !self.queue.is_empty() {
            self.queue_state.select(Some(0));
        }
    }

    fn add_album_or_artist_to_queue(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(node) = self.flat_library.get(selected) {
                match &node.item {
                    LibraryItem::Artist(_) => {
                        // Add all songs from this artist
                        self.add_all_songs_from_artist(selected);
                    }
                    LibraryItem::Album(_, _) => {
                        // Add all songs from this album
                        self.add_all_songs_from_album(selected);
                    }
                    LibraryItem::Song(_) => {
                        // Already handled by existing functionality
                    }
                }
            }
        }
    }

    fn add_all_songs_from_artist(&mut self, artist_index: usize) {
        if let Some(artist_node) = self.flat_library.get(artist_index) {
            if let LibraryItem::Artist(artist_name) = &artist_node.item {
                // Find all songs from this artist in the library
                for song in &self.songs {
                    if let Some(ref song_artist) = song.album_artist {
                        if song_artist == artist_name {
                            // Check if song is already in queue
                            if !self.queue.iter().any(|q| q.id == song.id) {
                                self.queue.push(song.clone());
                            }
                        }
                    }
                }

                // Update queue selection
                if self.queue_state.selected().is_none() && !self.queue.is_empty() {
                    self.queue_state.select(Some(0));
                }
            }
        }
    }

    fn add_all_songs_from_album(&mut self, album_index: usize) {
        if let Some(album_node) = self.flat_library.get(album_index) {
            if let LibraryItem::Album(artist_name, album_name) = &album_node.item {
                // Find all songs from this album in the library
                for song in &self.songs {
                    if let Some(ref song_artist) = song.album_artist {
                        if let Some(ref song_album) = song.album {
                            if song_artist == artist_name && song_album == album_name {
                                // Check if song is already in queue
                                if !self.queue.iter().any(|q| q.id == song.id) {
                                    self.queue.push(song.clone());
                                }
                            }
                        }
                    }
                }

                // Update queue selection
                if self.queue_state.selected().is_none() && !self.queue.is_empty() {
                    self.queue_state.select(Some(0));
                }
            }
        }
    }

    fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(ref sink) = self.sink {
            sink.set_volume(self.volume);
        }
    }

    fn adjust_volume(&mut self, delta: f32) {
        self.set_volume(self.volume + delta);
    }

    fn format_time(&self, milliseconds: u64) -> String {
        let total_seconds = milliseconds / 1000;
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}", minutes, seconds)
    }

    fn get_current_song_duration(&self) -> Option<u64> {
        self.current_song
            .as_ref()
            .and_then(|song| song.run_time_ticks)
            .map(|ticks| ticks / 10_000) // Convert ticks to milliseconds
    }

    fn is_song_finished(&self) -> bool {
        if let Some(ref sink) = self.sink {
            sink.empty() // Returns true if the sink has no more audio to play
        } else {
            false
        }
    }

    async fn play_next_in_queue(&mut self) -> Result<()> {
        if !self.queue.is_empty() {
            // Find the current song in the queue and remove it
            if let Some(ref current_song) = self.current_song {
                if let Some(current_index) = self
                    .queue
                    .iter()
                    .position(|song| song.id == current_song.id)
                {
                    // Remove the finished song from the queue
                    self.queue.remove(current_index);

                    // Adjust queue selection
                    if self.queue.is_empty() {
                        self.queue_state.select(None);
                        self.stop_current_song();
                        return Ok(());
                    }

                    // Play the next song (which is now at the same index)
                    if current_index < self.queue.len() {
                        let next_song = self.queue[current_index].clone();
                        self.stop_current_song();
                        self.queue_state.select(Some(current_index));
                        self.play_song(&next_song).await?;
                        return Ok(());
                    } else {
                        // If we were at the end, play the last song
                        let last_index = self.queue.len() - 1;
                        let next_song = self.queue[last_index].clone();
                        self.stop_current_song();
                        self.queue_state.select(Some(last_index));
                        self.play_song(&next_song).await?;
                        return Ok(());
                    }
                }
            }

            // If current song not found in queue, play first in queue
            let first_song = self.queue[0].clone();
            self.stop_current_song();
            self.queue_state.select(Some(0));
            self.play_song(&first_song).await?;
        }
        Ok(())
    }

    fn switch_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Library => ActivePanel::Queue,
            ActivePanel::Queue => ActivePanel::Library,
        };
    }

    fn toggle_queue_item(&mut self) {
        match self.active_panel {
            ActivePanel::Library => {
                if let Some(selected) = self.list_state.selected() {
                    if let Some(node) = self.flat_library.get(selected) {
                        if let LibraryItem::Song(song) = &node.item {
                            // Check if song is already in queue
                            if let Some(queue_index) =
                                self.queue.iter().position(|q| q.id == song.id)
                            {
                                self.remove_from_queue(queue_index);
                            } else {
                                self.add_to_queue(song.clone());
                            }
                        }
                    }
                }
            }
            ActivePanel::Queue => {
                if let Some(selected) = self.queue_state.selected() {
                    self.remove_from_queue(selected);
                }
            }
        }
    }

    fn navigate_queue_up(&mut self) {
        if !self.queue.is_empty() {
            let i = match self.queue_state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.queue.len() - 1
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            self.queue_state.select(Some(i));
        }
    }

    fn navigate_queue_down(&mut self) {
        if !self.queue.is_empty() {
            let i = match self.queue_state.selected() {
                Some(i) => {
                    if i >= self.queue.len() - 1 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            self.queue_state.select(Some(i));
        }
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

    fn navigate_up(&mut self) {
        match self.active_panel {
            ActivePanel::Library => {
                if !self.flat_library.is_empty() {
                    let i = match self.list_state.selected() {
                        Some(i) => {
                            if i == 0 {
                                self.flat_library.len() - 1
                            } else {
                                i - 1
                            }
                        }
                        None => 0,
                    };
                    self.list_state.select(Some(i));
                }
            }
            ActivePanel::Queue => {
                self.navigate_queue_up();
            }
        }
    }

    fn navigate_down(&mut self) {
        match self.active_panel {
            ActivePanel::Library => {
                if !self.flat_library.is_empty() {
                    let i = match self.list_state.selected() {
                        Some(i) => {
                            if i >= self.flat_library.len() - 1 {
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
            ActivePanel::Queue => {
                self.navigate_queue_down();
            }
        }
    }

    fn navigate_right(&mut self) {
        match self.active_panel {
            ActivePanel::Library => {
                if let Some(selected) = self.list_state.selected() {
                    if let Some(node) = self.flat_library.get(selected) {
                        match &node.item {
                            LibraryItem::Artist(_) | LibraryItem::Album(_, _) => {
                                // Find the corresponding node in the tree and expand it
                                self.expand_node_in_tree(selected);
                                self.flatten_library();

                                // Adjust selection if needed
                                if selected >= self.flat_library.len() {
                                    self.list_state.select(Some(self.flat_library.len() - 1));
                                }
                            }
                            LibraryItem::Song(_) => {
                                // Songs can't be expanded
                            }
                        }
                    }
                }
            }
            ActivePanel::Queue => {
                // Queue doesn't have expandable items
            }
        }
    }

    fn navigate_left(&mut self) {
        match self.active_panel {
            ActivePanel::Library => {
                if let Some(selected) = self.list_state.selected() {
                    if let Some(node) = self.flat_library.get(selected) {
                        match &node.item {
                            LibraryItem::Album(_, _) | LibraryItem::Song(_) => {
                                // Find the corresponding node in the tree and collapse it
                                self.collapse_node_in_tree(selected);
                                self.flatten_library();

                                // Adjust selection if needed
                                if selected >= self.flat_library.len() {
                                    self.list_state.select(Some(self.flat_library.len() - 1));
                                }
                            }
                            LibraryItem::Artist(_) => {
                                // Artists can't be collapsed further - do nothing
                            }
                        }
                    }
                }
            }
            ActivePanel::Queue => {
                // Queue doesn't have collapsible items
            }
        }
    }

    fn expand_node_in_tree(&mut self, flat_index: usize) {
        if let Some(node) = self.flat_library.get(flat_index) {
            match &node.item {
                LibraryItem::Artist(artist_name) => {
                    // Find and expand the artist in the tree
                    for artist_node in &mut self.library_tree {
                        if let LibraryItem::Artist(name) = &artist_node.item {
                            if name == artist_name {
                                artist_node.toggle_expansion();
                                break;
                            }
                        }
                    }
                }
                LibraryItem::Album(artist_name, album_name) => {
                    // Find and expand the album in the tree
                    for artist_node in &mut self.library_tree {
                        if let LibraryItem::Artist(name) = &artist_node.item {
                            if name == artist_name {
                                for album_node in &mut artist_node.children {
                                    if let LibraryItem::Album(_, album) = &album_node.item {
                                        if album == album_name {
                                            album_node.toggle_expansion();
                                            break;
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
                LibraryItem::Song(_) => {
                    // Songs can't be expanded
                }
            }
        }
    }

    fn collapse_node_in_tree(&mut self, flat_index: usize) {
        if let Some(node) = self.flat_library.get(flat_index) {
            match &node.item {
                LibraryItem::Album(artist_name, album_name) => {
                    // Find and collapse the album in the tree
                    for artist_node in &mut self.library_tree {
                        if let LibraryItem::Artist(name) = &artist_node.item {
                            if name == artist_name {
                                for album_node in &mut artist_node.children {
                                    if let LibraryItem::Album(_, album) = &album_node.item {
                                        if album == album_name {
                                            album_node.toggle_expansion();
                                            break;
                                        }
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
                LibraryItem::Song(_) => {
                    // Find and collapse the parent album
                    for artist_node in &mut self.library_tree {
                        for album_node in &mut artist_node.children {
                            for song_node in &album_node.children {
                                if let LibraryItem::Song(song) = &song_node.item {
                                    if let LibraryItem::Song(selected_song) = &node.item {
                                        if song.id == selected_song.id {
                                            album_node.toggle_expansion();
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                LibraryItem::Artist(_) => {
                    // Artists can't be collapsed further
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let (main_chunk, status_chunk) = {
        let (main_chunks, has_title) =
            if matches!(
                app.input_mode,
                InputMode::ServerUrl | InputMode::Username | InputMode::Password
            ) || matches!(app.loading_state, LoadingState::LoadingSongs { .. })
            {
                (
                    Layout::default()
                        .direction(Direction::Vertical)
                        .margin(1)
                        .constraints(
                            [
                                Constraint::Length(3), // Title
                                Constraint::Min(0),    // Main
                                Constraint::Length(3), // Status
                            ]
                            .as_ref(),
                        )
                        .split(f.size()),
                    true,
                )
            } else {
                (
                    Layout::default()
                        .direction(Direction::Vertical)
                        .margin(1)
                        .constraints(
                            [
                                Constraint::Min(0),    // Main
                                Constraint::Length(3), // Status
                            ]
                            .as_ref(),
                        )
                        .split(f.size()),
                    false,
                )
            };
        if has_title {
            let title = Paragraph::new(format!(
                "aiTunes v{} - By Sigvaldr",
                env!("CARGO_PKG_VERSION")
            ))
            .style(THEME.title_style())
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .style(THEME.primary_style()),
            );

            f.render_widget(title, main_chunks[0]);
            (main_chunks[1], main_chunks[2])
        } else {
            (main_chunks[0], main_chunks[1])
        }
    };

    match app.input_mode {
        InputMode::ServerUrl | InputMode::Username | InputMode::Password => {
            render_login_screen(f, main_chunk, app);
        }
        InputMode::SongList => match &app.loading_state {
            LoadingState::NotLoading => {
                render_main_content(f, main_chunk, app);
            }
            LoadingState::LoadingSongs { progress, total } => {
                render_loading_screen(f, main_chunk, *progress, *total);
            }
        },
    }
    render_status_bar(f, status_chunk, app);

    // Show help menu if enabled
    if app.show_help {
        render_help_menu(f, f.size());
    }
}

fn render_login_screen(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(area);

    // Server URL input
    let server_url_style = if app.input_mode == InputMode::ServerUrl {
        THEME.title_style()
    } else {
        THEME.primary_style()
    };
    let server_url = Paragraph::new(format!("Server URL: {}", app.server_url_input))
        .style(server_url_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Server URL")
                .style(THEME.primary_style()),
        );
    f.render_widget(server_url, chunks[0]);

    // Username input
    let username_style = if app.input_mode == InputMode::Username {
        THEME.title_style()
    } else {
        THEME.primary_style()
    };
    let username = Paragraph::new(format!("Username: {}", app.username_input))
        .style(username_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Username")
                .style(THEME.primary_style()),
        );
    f.render_widget(username, chunks[1]);

    // Password input
    let password_style = if app.input_mode == InputMode::Password {
        THEME.title_style()
    } else {
        THEME.primary_style()
    };
    let password_display = "*".repeat(app.password_input.len());
    let password = Paragraph::new(format!("Password: {}", password_display))
        .style(password_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Password")
                .style(THEME.primary_style()),
        );
    f.render_widget(password, chunks[2]);

    // Error message
    if let Some(ref error) = app.error_message {
        let error_area = Rect::new(area.x, area.y + 9, area.width, 3);
        let error_widget = Paragraph::new(error.as_str())
            .style(THEME.error_style())
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Error")
                    .style(THEME.error_style()),
            );
        f.render_widget(Clear, error_area);
        f.render_widget(error_widget, error_area);
    }
}

fn render_loading_screen(f: &mut Frame, area: Rect, progress: usize, total: usize) {
    let percentage = if total > 0 {
        (progress * 100) / total
    } else {
        0
    };
    let progress_text = format!(
        "Loading songs... {} / {} ({}%)",
        progress, total, percentage
    );

    // Create a progress bar
    let progress_width = if total > 0 {
        (area.width as usize * progress) / total
    } else {
        0
    };
    let progress_bar = "█".repeat(progress_width as usize);
    let remaining_bar = "░".repeat((area.width as usize).saturating_sub(progress_width as usize));

    let loading_widget = Paragraph::new(vec![
        Line::from(progress_text),
        Line::from(""),
        Line::from(format!("{}{}", progress_bar, remaining_bar)),
        Line::from(""),
        Line::from("Please wait while your music library loads..."),
    ])
    .style(THEME.primary_style())
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Loading Music Library")
            .style(THEME.primary_style()),
    );

    f.render_widget(loading_widget, area);
}

fn render_main_content(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(area);

    render_song_list(f, chunks[0], app);
    render_queue(f, chunks[1], app);
}

fn render_song_list(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .flat_library
        .iter()
        .map(|node| {
            let indent = "  ".repeat(node.get_indent_level());
            let display_name = node.get_display_name();

            let (prefix, style) = match &node.item {
                LibraryItem::Artist(_) => {
                    let symbol = if node.is_expanded() { "▼" } else { "▶" };
                    (format!("{}{} ", indent, symbol), THEME.title_style())
                }
                LibraryItem::Album(_, _) => {
                    let symbol = if node.is_expanded() { "▼" } else { "▶" };
                    (format!("{}{} ", indent, symbol), THEME.secondary_style())
                }
                LibraryItem::Song(song) => {
                    let duration = if let Some(ticks) = song.run_time_ticks {
                        let seconds = ticks / 10_000_000;
                        let minutes = seconds / 60;
                        let remaining_seconds = seconds % 60;
                        format!("{:02}:{:02}", minutes, remaining_seconds)
                    } else {
                        "Unknown".to_string()
                    };

                    let _artist = song.album_artist.as_deref().unwrap_or("Unknown Artist");
                    let album = song.album.as_deref().unwrap_or("Unknown Album");

                    return ListItem::new(Line::from(vec![
                        Span::styled(format!("{}  {}", indent, song.name), THEME.primary_style()),
                        Span::raw(" "),
                        Span::styled(format!("[{}]", album), THEME.muted_style()),
                        Span::raw(" "),
                        Span::styled(format!("({})", duration), THEME.accent_style()),
                    ]));
                }
            };

            ListItem::new(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(display_name, style),
            ]))
        })
        .collect();

    let songs_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Music Library")
                .style(THEME.primary_style()),
        )
        .highlight_style(THEME.highlight_style());

    f.render_stateful_widget(songs_list, area, &mut app.list_state.clone());
}

fn render_queue(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .queue
        .iter()
        .enumerate()
        .map(|(i, song)| {
            let duration = if let Some(ticks) = song.run_time_ticks {
                let seconds = ticks / 10_000_000;
                let minutes = seconds / 60;
                let remaining_seconds = seconds % 60;
                format!("{:02}:{:02}", minutes, remaining_seconds)
            } else {
                "Unknown".to_string()
            };

            let artist = song.album_artist.as_deref().unwrap_or("Unknown Artist");

            ListItem::new(Line::from(vec![
                Span::styled(format!("{}. {}", i + 1, song.name), THEME.primary_style()),
                Span::raw(" "),
                Span::styled(format!("[{}]", artist), THEME.muted_style()),
                Span::raw(" "),
                Span::styled(format!("({})", duration), THEME.accent_style()),
            ]))
        })
        .collect();

    let queue_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Queue")
                .style(THEME.primary_style()),
        )
        .highlight_style(THEME.highlight_style());

    f.render_stateful_widget(queue_list, area, &mut app.queue_state.clone());
}

fn render_status_bar(f: &mut Frame, area: Rect, app: &App) {
    // Add border around the entire status bar first
    let status_block = Block::default()
        .borders(Borders::ALL)
        .title("Status")
        .title_style(THEME.title_style())
        .style(THEME.primary_style());
    f.render_widget(status_block, area);

    // Create horizontal layout for better spacing inside the bordered area
    let inner_area = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage(50), // Song info (left side)
                Constraint::Percentage(25), // Time info (center-left)
                Constraint::Percentage(25), // Volume info (right side)
            ]
            .as_ref(),
        )
        .split(inner_area);

    let song_info = if let Some(ref song) = app.current_song {
        let artist = song.album_artist.as_deref().unwrap_or("Unknown Artist");
        format!("{} - {}", song.name, artist)
    } else {
        "No song playing".to_string()
    };

    let play_pause_text = if app.current_song.is_some() {
        if app.is_paused {
            "⏸️"
        } else {
            "▶️"
        }
    } else {
        "⏹️"
    };

    let time_text = if let Some(ref _song) = app.current_song {
        let current_time_str = app.format_time(app.current_time);
        let total_duration = app
            .get_current_song_duration()
            .map(|d| app.format_time(d))
            .unwrap_or_else(|| "Unknown".to_string());
        format!("{} / {}", current_time_str, total_duration)
    } else {
        "00:00 / 00:00".to_string()
    };

    let volume_percent = (app.volume * 100.0) as u32;
    let volume_text = format!("🔊 {}%", volume_percent);

    // Left side: Play/pause button and song info
    let left_text = format!("{} {}", play_pause_text, song_info);
    let left_widget = Paragraph::new(left_text)
        .style(THEME.primary_style())
        .alignment(Alignment::Left);
    f.render_widget(left_widget, chunks[0]);

    // Center: Time info
    let time_widget = Paragraph::new(time_text)
        .style(THEME.accent_style())
        .alignment(Alignment::Center);
    f.render_widget(time_widget, chunks[1]);

    // Right side: Volume info
    let volume_widget = Paragraph::new(volume_text)
        .style(THEME.accent_style())
        .alignment(Alignment::Right);
    f.render_widget(volume_widget, chunks[2]);
}

fn render_help_menu(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from("aiTunes - Key Bindings"),
        Line::from(""),
        Line::from("  ↑/↓     Navigate up/down"),
        Line::from("  ←/→     Expand/collapse folders"),
        Line::from("  Tab     Switch between Library and Queue"),
        Line::from("  Enter   Play selected song/queue item"),
        Line::from("  Space   Pause/Resume"),
        Line::from("  +/-     Change volume"),
        Line::from("  PgUp/Dn Change volume"),
        Line::from("  Q       Add/Remove selection from queue"),
        Line::from("  S       Shuffle queue"),
        Line::from("  H/?      Show/Hide this help menu"),
        Line::from("  Esc     Exit application"),
    ];

    let help_widget = Paragraph::new(help_text)
        .style(THEME.primary_style())
        .alignment(Alignment::Left)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Help")
                .title_style(THEME.title_style())
                .style(THEME.primary_style()),
        );

    // Center the help menu on screen
    let help_area = Rect::new(
        area.x + area.width / 4,
        area.y + area.height / 4,
        area.width / 2,
        area.height / 2,
    );

    f.render_widget(Clear, help_area);
    f.render_widget(help_widget, help_area);
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
            // Start loading and show progress
            if let Err(e) = app.load_songs_with_progress(&mut terminal).await {
                app.error_message = Some(format!("Failed to load songs: {}", e));
            }
        }
    }

    // Main loop
    loop {
        terminal.draw(|f| ui(f, &app))?;

        // Update current time for the status bar
        if app.input_mode == InputMode::SongList {
            app.update_current_time();
        }

        // Check for autoplay - if current song finished and we have songs in queue
        if app.input_mode == InputMode::SongList && app.is_song_finished() && !app.queue.is_empty()
        {
            if let Err(e) = app.play_next_in_queue().await {
                app.error_message = Some(format!("Autoplay failed: {}", e));
            }
            // Small delay to prevent rapid autoplay checks
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Use non-blocking event reading with timeout
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Tab => {
                            if app.input_mode == InputMode::SongList {
                                app.switch_panel();
                            }
                        }
                        KeyCode::Char('q') => {
                            if app.input_mode == InputMode::SongList {
                                app.toggle_queue_item();
                                app.add_album_or_artist_to_queue();
                            }
                        }
                        KeyCode::PageUp => {
                            if app.input_mode == InputMode::SongList {
                                app.adjust_volume(0.1);
                            }
                        }
                        KeyCode::PageDown => {
                            if app.input_mode == InputMode::SongList {
                                app.adjust_volume(-0.1);
                            }
                        }
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
                                    ' ' => {
                                        app.pause_unpause();
                                    }
                                    '+' | '=' => {
                                        app.adjust_volume(0.1);
                                    }
                                    '-' => {
                                        app.adjust_volume(-0.1);
                                    }
                                    's' | 'S' => {
                                        app.shuffle_queue();
                                    }
                                    '?' | '/' | 'h' | 'H' => {
                                        app.toggle_help();
                                    }
                                     _ => {
                                    }
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
                                        app.error_message =
                                            Some(format!("Authentication failed: {}", e));
                                    } else {
                                        app.input_mode = InputMode::SongList;
                                        if let Err(e) = app.load_songs().await {
                                            app.error_message =
                                                Some(format!("Failed to load songs: {}", e));
                                        }
                                    }
                                }
                                InputMode::SongList => match app.active_panel {
                                    ActivePanel::Library => {
                                        if let Some(selected) = app.list_state.selected() {
                                            if let Some(node) = app.flat_library.get(selected) {
                                                match &node.item {
                                                    LibraryItem::Song(song) => {
                                                        let _song_name = song.name.clone();
                                                        let song_clone = song.clone();
                                                        app.stop_current_song();
                                                        app.add_to_queue(song_clone.clone());
                                                        if let Err(e) =
                                                            app.play_song(&song_clone).await
                                                        {
                                                            app.error_message = Some(format!(
                                                                "Failed to play song: {}",
                                                                e
                                                            ));
                                                        }
                                                    }
                                                    LibraryItem::Artist(_)
                                                    | LibraryItem::Album(_, _) => {
                                                        app.navigate_right();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    ActivePanel::Queue => {
                                        if let Some(queue_selected) = app.queue_state.selected() {
                                            if let Some(song) = app.queue.get(queue_selected) {
                                                let song_clone = song.clone();
                                                app.stop_current_song();
                                                if let Err(e) = app.play_song(&song_clone).await {
                                                    app.error_message = Some(format!(
                                                        "Failed to play queue song: {}",
                                                        e
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                        KeyCode::Up => {
                            if app.input_mode == InputMode::SongList {
                                app.navigate_up();
                            }
                        }
                        KeyCode::Down => {
                            if app.input_mode == InputMode::SongList {
                                app.navigate_down();
                            }
                        }
                        KeyCode::Left => {
                            if app.input_mode == InputMode::SongList {
                                app.navigate_left();
                            }
                        }
                        KeyCode::Right => {
                            if app.input_mode == InputMode::SongList {
                                app.navigate_right();
                            }

                        }
                        _ => {}
                    }
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
