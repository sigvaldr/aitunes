# Windows Build Guide for aiTunes

This guide provides detailed instructions for building aiTunes on Windows 10/11.

## Prerequisites

### Required Software

1. **Visual Studio 2019 or later** with C++ support
   - Download from: https://visualstudio.microsoft.com/downloads/
   - During installation, make sure to select "Desktop development with C++"
   - This includes MSVC compiler and CMake

2. **Git for Windows**
   - Download from: https://git-scm.com/download/win
   - Required for vcpkg installation

3. **CMake** (optional, included with Visual Studio)
   - Download from: https://cmake.org/download/ if not using Visual Studio

### Alternative: MinGW-w64

If you prefer using MinGW-w64 instead of Visual Studio:

1. **MSYS2** (recommended)
   - Download from: https://www.msys2.org/
   - Install and update packages
   - Install required packages: `pacman -S mingw-w64-x86_64-gcc mingw-w64-x86_64-cmake`

2. **Or standalone MinGW-w64**
   - Download from: https://www.mingw-w64.org/downloads/

## Build Methods

### Method 1: Automated Build (Recommended)

This is the easiest method and handles all dependencies automatically.

1. **Clone the repository**
   ```cmd
   git clone https://github.com/sigvaldr/aitunes.git
   cd aitunes
   ```

2. **Run the automated build script**
   ```cmd
   build_windows.bat
   ```
   
   Or using PowerShell:
   ```powershell
   Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
   .\build_windows.ps1
   ```

The script will:
- Install vcpkg if not present
- Install all required dependencies (curl, pdcurses, nlohmann-json)
- Download header files (dr_mp3, miniaudio)
- Build using both CMake and direct g++ compilation
- Create executables in the `dist` folder

### Method 2: Manual Build with vcpkg

If you prefer more control over the build process:

1. **Install vcpkg**
   ```cmd
   git clone https://github.com/Microsoft/vcpkg.git C:\vcpkg
   C:\vcpkg\bootstrap-vcpkg.bat
   C:\vcpkg\vcpkg integrate install
   ```

2. **Install dependencies**
   ```cmd
   C:\vcpkg\vcpkg install curl:x64-windows
   C:\vcpkg\vcpkg install pdcurses:x64-windows
   C:\vcpkg\vcpkg install nlohmann-json:x64-windows
   ```

3. **Download header dependencies**
   ```cmd
   download_deps.bat
   ```

4. **Build using CMake**
   ```cmd
   mkdir build
   cd build
   cmake .. -DCMAKE_TOOLCHAIN_FILE=C:/vcpkg/scripts/buildsystems/vcpkg.cmake -DCMAKE_BUILD_TYPE=Release
   cmake --build . --config Release
   cd ..
   ```

5. **Or build directly with g++**
   ```cmd
   g++ src\main.cpp -o dist\aitunes.exe -std=c++17 -I"C:\vcpkg\installed\x64-windows\include" -L"C:\vcpkg\installed\x64-windows\lib" -lcurl -lpdcurses -lwinmm -lws2_32 -static-libgcc -static-libstdc++
   ```

### Method 3: Build with MSYS2/MinGW-w64

If you're using MSYS2:

1. **Install dependencies**
   ```bash
   pacman -S mingw-w64-x86_64-curl mingw-w64-x86_64-pdcurses mingw-w64-x86_64-nlohmann-json
   ```

2. **Download header dependencies**
   ```bash
   ./download_deps.sh
   ```

3. **Build**
   ```bash
   g++ src/main.cpp -o dist/aitunes.exe -std=c++17 -lcurl -lpdcurses -lwinmm -lws2_32
   ```

## Troubleshooting

### Common Issues

1. **"vcpkg not found" error**
   - Make sure Git is installed and in your PATH
   - Run the build script as Administrator if needed

2. **"cmake not found" error**
   - Install CMake or use Visual Studio which includes it
   - Add CMake to your system PATH

3. **"g++ not found" error**
   - Install MinGW-w64 or use Visual Studio
   - Add MinGW-w64 bin directory to your PATH

4. **"curl not found" error**
   - Make sure vcpkg installed curl successfully
   - Check that the vcpkg path in build scripts matches your installation

5. **"pdcurses not found" error**
   - Make sure vcpkg installed pdcurses successfully
   - PDCurses is the Windows equivalent of ncurses

6. **Build fails with linker errors**
   - Make sure all dependencies are installed for the correct architecture (x64-windows)
   - Try building with static linking flags

### Environment Variables

You may need to set these environment variables:

```cmd
set VCPKG_ROOT=C:\vcpkg
set PATH=%PATH%;C:\vcpkg\installed\x64-windows\bin
```

### Alternative Dependency Management

If vcpkg doesn't work for you:

1. **Use Conan package manager**
2. **Download libraries manually** and set include/lib paths
3. **Use vcpkg with different triplet** (e.g., x86-windows for 32-bit)

## Running the Application

After successful build:

1. **Navigate to the dist folder**
   ```cmd
   cd dist
   ```

2. **Run the application**
   ```cmd
   aitunes.exe
   ```

3. **First run setup**
   - Enter your Jellyfin server URL
   - Enter your username and password
   - The app will authenticate and load your music library

## File Structure After Build

```
aitunes/
├── dist/
│   ├── aitunes.exe          # CMake build
│   └── aitunes_gcc.exe      # Direct g++ build
├── build/                   # CMake build directory
├── include/                 # Header files
│   ├── dr_mp3.h
│   └── miniaudio.h
└── src/
    └── main.cpp
```

## Performance Notes

- The CMake build is generally more optimized
- Static linking reduces dependency issues
- PDCurses provides native Windows console support
- Audio playback uses miniaudio which supports Windows audio APIs

## Support

If you encounter issues:

1. Check the troubleshooting section above
2. Ensure all prerequisites are installed correctly
3. Try the automated build script first
4. Check that your Jellyfin server is accessible
5. Verify your Windows version supports the required features 