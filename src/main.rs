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
}

#[derive(Debug, Clone)]
enum LoadingState {
    NotLoading,
    LoadingSongs { progress: usize, total: usize },
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
        
        // Start loading state
        self.loading_state = LoadingState::LoadingSongs { progress: 0, total: 0 };
        
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
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!("Failed to load songs: {}", error_text));
        }

        let count_response: serde_json::Value = response.json().await?;
        let total_count = count_response["TotalRecordCount"].as_u64().unwrap_or(0) as usize;
        
        self.loading_state = LoadingState::LoadingSongs { progress: 0, total: total_count };
        
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
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                return Err(anyhow!("Failed to load songs: {}", error_text));
            }

            let songs_response: serde_json::Value = response.json().await?;
            
            if let Some(items) = songs_response["Items"].as_array() {
                for item in items {
                    if let Ok(jellyfin_item) = serde_json::from_value::<JellyfinItem>(item.clone()) {
                        all_songs.push(jellyfin_item);
                    }
                }
            }
            
            start_index += batch_size;
            self.loading_state = LoadingState::LoadingSongs { 
                progress: start_index.min(total_count), 
                total: total_count 
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

    async fn load_songs_with_progress(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        let auth = self.auth.as_ref().ok_or_else(|| anyhow!("Not authenticated"))?;
        let client = Client::new();
        
        // Start loading state
        self.loading_state = LoadingState::LoadingSongs { progress: 0, total: 0 };
        
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
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!("Failed to load songs: {}", error_text));
        }

        let count_response: serde_json::Value = response.json().await?;
        let total_count = count_response["TotalRecordCount"].as_u64().unwrap_or(0) as usize;
        
        self.loading_state = LoadingState::LoadingSongs { progress: 0, total: total_count };
        
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
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                return Err(anyhow!("Failed to load songs: {}", error_text));
            }

            let songs_response: serde_json::Value = response.json().await?;
            
            if let Some(items) = songs_response["Items"].as_array() {
                for item in items {
                    if let Ok(jellyfin_item) = serde_json::from_value::<JellyfinItem>(item.clone()) {
                        all_songs.push(jellyfin_item);
                    }
                }
            }
            
            start_index += batch_size;
            self.loading_state = LoadingState::LoadingSongs { 
                progress: start_index.min(total_count), 
                total: total_count 
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
            let artist = song.album_artist.as_deref().unwrap_or("Unknown Artist").to_string();
            let album = song.album.as_deref().unwrap_or("Unknown Album").to_string();
            
            artists.entry(artist)
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
                let mut album_node = LibraryNode::new(LibraryItem::Album(artist_name.clone(), album_name.clone()));
                
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
        let auth = self.auth.as_ref().ok_or_else(|| anyhow!("Not authenticated"))?;
        let client = Client::new();
        
        // Try different URL formats for better compatibility
        let urls_to_try = vec![
            format!("{}/Items/{}/Download", self.credentials.as_ref().unwrap().server_url, song.id),
            format!("{}/Audio/{}/stream", self.credentials.as_ref().unwrap().server_url, song.id),
            format!("{}/Audio/{}/stream?api_key={}", self.credentials.as_ref().unwrap().server_url, song.id, auth.access_token),
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
                let _error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
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
                    Ok(source) => {
                        source
                    }
                    Err(e2) => {
                        return Err(anyhow!("Unrecognized format: {} (original: {})", e2, e));
                    }
                }
            }
        };
        
        sink.append(source);
        
        self.sink = Some(sink);
        self._stream = Some(_stream);
        self.current_song = Some(song.clone());

        Ok(())
    }

    fn stop_current_song(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self._stream = None;
        self.current_song = None;
        self.is_paused = false;
    }
    
    fn pause_unpause(&mut self) {
        if let Some(ref sink) = self.sink {
            if self.is_paused {
                sink.play();
                self.is_paused = false;
            } else {
                sink.pause();
                self.is_paused = true;
            }
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

    fn navigate_down(&mut self) {
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

    fn navigate_up(&mut self) {
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
    
    fn navigate_right(&mut self) {
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
    
    fn navigate_left(&mut self) {
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
                        // Artists can't be collapsed further
                    }
                }
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
            match &app.loading_state {
                LoadingState::NotLoading => {
                    render_song_list(f, chunks[1], app);
                }
                LoadingState::LoadingSongs { progress, total } => {
                    render_loading_screen(f, chunks[1], *progress, *total);
                }
            }
        }
    }

    // Status bar
    let status_text = match app.input_mode {
        InputMode::ServerUrl => "Enter Jellyfin server URL (e.g., http://localhost:8096)".to_string(),
        InputMode::Username => "Enter username".to_string(),
        InputMode::Password => "Enter password".to_string(),
        InputMode::SongList => {
            if let Some(ref song) = app.current_song {
                let status = if app.is_paused { "Paused" } else { "Now playing" };
                format!("{}: {} - {}", status, song.name, song.album_artist.as_deref().unwrap_or("Unknown"))
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

fn render_loading_screen(f: &mut Frame, area: Rect, progress: usize, total: usize) {
    let percentage = if total > 0 { (progress * 100) / total } else { 0 };
    let progress_text = format!("Loading songs... {} / {} ({}%)", progress, total, percentage);
    
    // Create a progress bar
    let progress_width = if total > 0 { (area.width as usize * progress) / total } else { 0 };
    let progress_bar = "█".repeat(progress_width as usize);
    let remaining_bar = "░".repeat((area.width as usize).saturating_sub(progress_width as usize));
    
    let loading_widget = Paragraph::new(vec![
        Line::from(progress_text),
        Line::from(""),
        Line::from(format!("{}{}", progress_bar, remaining_bar)),
        Line::from(""),
        Line::from("Please wait while your music library loads..."),
    ])
    .style(Style::default().fg(Color::Yellow))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::ALL).title("Loading Music Library"));
    
    f.render_widget(loading_widget, area);
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
                    (format!("{}{} ", indent, symbol), Style::default().fg(Color::Cyan))
                }
                LibraryItem::Album(_, _) => {
                    let symbol = if node.is_expanded() { "▼" } else { "▶" };
                    (format!("{}{} ", indent, symbol), Style::default().fg(Color::Green))
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
                        Span::styled(
                            format!("{}  {}", indent, song.name),
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
        .block(Block::default().borders(Borders::ALL).title("Music Library"))
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
            // Start loading and show progress
            if let Err(e) = app.load_songs_with_progress(&mut terminal).await {
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
                                ' ' => {
                                    app.pause_unpause();
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
                                    if let Some(node) = app.flat_library.get(selected) {
                                        match &node.item {
                                            LibraryItem::Song(song) => {
                                                let _song_name = song.name.clone();
                                                let song_clone = song.clone();
                                                app.stop_current_song();
                                                if let Err(e) = app.play_song(&song_clone).await {
                                                    app.error_message = Some(format!("Failed to play song: {}", e));
                                                } else {
                                                }
                                            }
                                            LibraryItem::Artist(_) | LibraryItem::Album(_, _) => {
                                                // Expand/collapse the node
                                                app.navigate_right();
                                            }
                                        }
                                    }
                                }
                            }
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