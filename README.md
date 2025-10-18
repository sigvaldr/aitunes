<h1 align="center">
  <br>
  <img src="https://raw.githubusercontent.com/sigvaldr/aitunes/refs/heads/master/logo.png" alt="aiTunes" width="200">
  <br>
  aiTunes
  <br>
</h1>

# aiTunes - Terminal Jellyfin Music Player

A terminal-based music player that streams music from your Jellyfin server. Built with Rust and featuring a clean, intuitive interface.

## Features

- 🎵 Stream music directly from your Jellyfin server
- 🔐 Credential storage (saved locally, not prompted every time)
- 🎨 Beautiful terminal UI with song browsing
- ⌨️ Intuitive keyboard shortcuts for easy navigation
- 🔊 High-quality audio playback using Rodio
- 📱 Cross-platform support

## Installation

Simply download and run the correct binary for your operating system

## Usage

### First Run

1. Launch the application

2. Enter your Jellyfin server details when prompted:
   - **Server URL**: Your Jellyfin server address (e.g., `http://localhost:8096` or `https://your-server.com`)
   - **Username**: Your Jellyfin username
   - **Password**: Your Jellyfin password

3. The application will authenticate and load your music library

### Navigation
Press `?` or `/` for this information within the application

↑/↓     Navigate up/down
←/→     Expand/collapse folders
Tab     Switch between Library and Queue
Enter   Play selected song/queue item
Space   Pause/Resume
+/-     Change volume
PgUp/Dn Change volume
Q       Add/Remove selection from queue
S       Shuffle queue
C       Clear queue
Esc     Exit application

### Credential Storage

Your credentials are securely stored in your system's config directory:
- **Windows**: `%APPDATA%\aitunes\credentials.json`
- **macOS**: `~/Library/Application Support/aitunes/credentials.json`
- **Linux**: `~/.config/aitunes/credentials.json`

The credentials are stored in plain text JSON format. For enhanced security, consider using environment variables or a more secure storage method.

## Build from Source

### Prerequisites

- The latest version of Rust and Cargo installed on your system

```bash
git clone https://github.com/sigvaldr/aitunes.git
cd aitunes
cargo build --release
```

The executable will be created at `target/release/aitunes` (or `target/release/aitunes.exe` on Windows).

### Crates Used

- **reqwest**: HTTP client for Jellyfin API communication
- **rodio**: Audio playback engine
- **ratatui**: Terminal UI framework
- **crossterm**: Cross-platform terminal manipulation
- **serde**: Serialization/deserialization
- **tokio**: Async runtime

### Jellyfin API Integration

The application uses the Jellyfin REST API to:
- Authenticate users via `/Users/authenticatebyname`
- Retrieve music items via `/Users/{userId}/Items`
- Stream audio via `/Audio/{itemId}/Download`

### Audio Streaming

Songs are streamed directly from your Jellyfin server and played using Rodio's audio engine, which supports various audio formats including MP3, FLAC, and more.

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## License

This project is licensed under the MIT License - see the LICENSE file for details.