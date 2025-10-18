<h1 align="center">
  <br>
  <img src="https://raw.githubusercontent.com/sigvaldr/aitunes/refs/heads/master/img/logo.png" alt="aiTunes" width="200">
  <br>
  aiTunes
  <br>
</h1>

# aiTunes - Terminal Jellyfin Music Player

A terminal-based music player that streams music from your Jellyfin server. Built with Rust and featuring a clean, intuitive interface.

## Features

- 🎵 Stream music directly from your Jellyfin server
- 🔐 Secure credential storage (saved locally, not prompted every time)
- 🎨 Beautiful terminal UI with song browsing
- ⌨️ Keyboard shortcuts for easy navigation
- 🔊 High-quality audio playback using Rodio
- 📱 Cross-platform support

## Installation

### Prerequisites

- Rust 1.70+ installed on your system
- A running Jellyfin server with music library

### Build from Source

```bash
git clone <your-repo-url>
cd aitunes
cargo build --release
```

The executable will be created at `target/release/aitunes` (or `target/release/aitunes.exe` on Windows).

## Usage

### First Run

1. Launch the application:
   ```bash
   cargo run
   # or if built:
   ./target/release/aitunes
   ```

2. Enter your Jellyfin server details when prompted:
   - **Server URL**: Your Jellyfin server address (e.g., `http://localhost:8096` or `https://your-server.com`)
   - **Username**: Your Jellyfin username
   - **Password**: Your Jellyfin password

3. The application will authenticate and load your music library

### Navigation

Once authenticated, you can navigate the music library:

- **Arrow Keys** or **j/k**: Navigate up/down through songs
- **Enter**: Play the selected song
- **Space**: Stop current playback
- **Esc** or **q**: Exit the application

### Credential Storage

Your credentials are securely stored in your system's config directory:
- **Windows**: `%APPDATA%\aitunes\credentials.json`
- **macOS**: `~/Library/Application Support/aitunes/credentials.json`
- **Linux**: `~/.config/aitunes/credentials.json`

The credentials are stored in plain text JSON format. For enhanced security, consider using environment variables or a more secure storage method.

## Technical Details

### Dependencies

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

## Troubleshooting

### Common Issues

1. **Authentication Failed**
   - Verify your server URL is correct and accessible
   - Check your username and password
   - Ensure your Jellyfin server is running

2. **No Songs Loaded**
   - Verify your Jellyfin library contains audio files
   - Check that your user has access to the music library
   - Ensure the library is properly configured in Jellyfin

3. **Audio Playback Issues**
   - Check your system's audio output
   - Verify the audio file format is supported
   - Try restarting the application

### Debug Mode

For debugging, you can run with more verbose output:
```bash
RUST_LOG=debug cargo run
```

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with the amazing Rust ecosystem
- Uses Jellyfin's excellent media server platform
- Inspired by terminal-based music players like cmus and ncmpcpp