<h1 align="center">
  <br>
  <img src="https://raw.githubusercontent.com/sigvaldr/aitunes/refs/heads/master/img/logo.png" alt="aiTunes" width="200">
  <br>
  aiTunes
  <br>
</h1>

<h4 align="center">A terminal-based music player, with 99% of the source being written by ChatGPT</h4>

<!-- <p align="center">
  <a href="https://badge.fury.io/js/electron-markdownify">
    <img src="https://badge.fury.io/js/electron-markdownify.svg"
         alt="Gitter">
  </a>
  <a href="https://gitter.im/amitmerchant1990/electron-markdownify"><img src="https://badges.gitter.im/amitmerchant1990/electron-markdownify.svg"></a>
  <a href="https://saythanks.io/to/bullredeyes@gmail.com">
      <img src="https://img.shields.io/badge/SayThanks.io-%E2%98%BC-1EAEDB.svg">
  </a>
  <a href="https://www.paypal.me/AmitMerchant">
    <img src="https://img.shields.io/badge/$-donate-ff69b4.svg?maxAge=2592000&amp;style=flat">
  </a>
</p> -->

<p align="center">
  <a href="#key-features">Key Features</a> •
  <a href="#how-to-use">How To Use</a> •
  <a href="#download">Download</a>
</p>

![screenshot](https://raw.githubusercontent.com/sigvaldr/aitunes/refs/heads/master/img/screenshot.png)

## Key Features

- Cross platform
  - Windows, macOS and Linux ready.
- Lightweight audio playback
  - Uses dr_mp3 for MP3 decoding and miniaudio for audio output
  - No heavy dependencies like libvlc
- Terminal-based interface
  - Full ncurses-based TUI with tree navigation
  - Queue management and shuffle functionality

## How To Use

### Prerequisites

- libcurl development headers
- ncurses development headers
- ALSA development headers (for Linux audio)
- pthread (usually included with gcc)

### Building

1. Clone the repository
2. Run the build script:
   ```bash
   chmod +x build.sh
   ./build.sh
   ```

The build script will automatically download the required dr_mp3 and miniaudio headers.

### Running

1. Run the compiled binary:
   ```bash
   ./dist/aitunes
   ```

2. On first run, you'll be prompted to enter your Jellyfin server details:
   - Server URL
   - Username
   - Password

3. The app will authenticate and load your music library.

### Controls

- **Navigation**: Arrow keys to move, Enter to expand/collapse folders
- **Playback**: Enter to play a track, Space to pause/resume
- **Volume**: Page Up/Down to adjust volume
- **Queue**: F to add tracks to queue, Tab to switch focus
- **Shuffle**: S to shuffle the queue
- **Quit**: Q to exit

## Download

You can [download](https://github.com/sigvaldr/aitunes/releases/) the latest installable version of aiTunes for Linux. (Windows and macOS soon™️)

## Emailware

aiTunes is an [emailware](https://en.wiktionary.org/wiki/emailware). If you have used this app and have anything at all to say, I'd like you send me an email at <me@sigvaldr.lol>. I'd love to hear your feedback!
