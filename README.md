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
  - Full PDCurses-based TUI with tree navigation (Windows) / ncurses-based TUI (Linux/macOS)
  - Queue management and shuffle functionality

## How To Use

### Prerequisites

#### Windows
- Visual Studio 2019 or later with C++ support, OR MinGW-w64
- Git (for vcpkg installation)
- CMake (optional, for CMake build)

#### Linux
- libcurl development headers
- ncurses development headers
- ALSA development headers (for Linux audio)
- pthread (usually included with gcc)

#### macOS
- Xcode Command Line Tools
- Homebrew (recommended for dependencies)

### Building

#### Windows 10/11

**Option 1: Automated Build (Recommended)**
1. Clone the repository
2. Run the automated build script:
   ```cmd
   build_windows.bat
   ```
   This script will:
   - Install vcpkg if not present
   - Install all required dependencies
   - Download header files
   - Build using both CMake and direct g++ compilation

**Option 2: Manual Build**
1. Install vcpkg:
   ```cmd
   git clone https://github.com/Microsoft/vcpkg.git C:\vcpkg
   C:\vcpkg\bootstrap-vcpkg.bat
   C:\vcpkg\vcpkg integrate install
   ```

2. Install dependencies:
   ```cmd
   C:\vcpkg\vcpkg install curl:x64-windows
   C:\vcpkg\vcpkg install pdcurses:x64-windows
   C:\vcpkg\vcpkg install nlohmann-json:x64-windows
   ```

3. Download dependencies:
   ```cmd
   download_deps.bat
   ```

4. Build using CMake:
   ```cmd
   mkdir build
   cd build
   cmake .. -DCMAKE_TOOLCHAIN_FILE=C:/vcpkg/scripts/buildsystems/vcpkg.cmake
   cmake --build . --config Release
   ```

5. Or build directly with g++:
   ```cmd
   g++ src\main.cpp -o dist\aitunes.exe -std=c++17 -I"C:\vcpkg\installed\x64-windows\include" -L"C:\vcpkg\installed\x64-windows\lib" -lcurl -lpdcurses -lwinmm -lws2_32 -static-libgcc -static-libstdc++
   ```

#### Linux

1. Clone the repository
2. Run the build script:
   ```bash
   chmod +x build.sh
   ./build.sh
   ```

#### macOS

1. Install dependencies via Homebrew:
   ```bash
   brew install curl ncurses nlohmann-json
   ```

2. Clone the repository and build:
   ```bash
   chmod +x build.sh
   ./build.sh
   ```

The build scripts will automatically download the required dr_mp3 and miniaudio headers.

### Running

#### Windows
1. Run the compiled binary:
   ```cmd
   dist\aitunes.exe
   ```

#### Linux/macOS
1. Run the compiled binary:
   ```bash
   ./dist/aitunes
   ```

#### First Run Setup
1. On first run, you'll be prompted to enter your Jellyfin server details:
   - Server URL
   - Username
   - Password

2. The app will authenticate and load your music library.

### Controls

- **Navigation**: Arrow keys to move, Enter to expand/collapse folders
- **Playback**: Enter to play a track, Space to pause/resume
- **Volume**: Page Up/Down to adjust volume
- **Queue**: F to add tracks to queue, Tab to switch focus
- **Shuffle**: S to shuffle the queue
- **Quit**: Q to exit

## Download

You can [download](https://github.com/sigvaldr/aitunes/releases/) the latest installable version of aiTunes for Linux. Windows and macOS builds are now available through the automated build process.

## Emailware

aiTunes is an [emailware](https://en.wiktionary.org/wiki/emailware). If you have used this app and have anything at all to say, I'd like you send me an email at <me@sigvaldr.lol>. I'd love to hear your feedback!
