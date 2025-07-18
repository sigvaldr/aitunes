@echo off
setlocal enabledelayedexpansion

echo ========================================
echo aiTunes Windows Build Script
echo ========================================

REM Check if vcpkg is installed
if not exist "C:\vcpkg\vcpkg.exe" (
    echo vcpkg not found. Installing vcpkg...
    git clone https://github.com/Microsoft/vcpkg.git C:\vcpkg
    C:\vcpkg\bootstrap-vcpkg.bat
    C:\vcpkg\vcpkg integrate install
)

REM Install required packages
echo Installing required packages...
C:\vcpkg\vcpkg install curl:x64-windows
C:\vcpkg\vcpkg install pdcurses:x64-windows
C:\vcpkg\vcpkg install nlohmann-json:x64-windows

REM Download dependencies if they don't exist
if not exist "include\dr_mp3.h" (
    echo Downloading dependencies...
    call download_deps.bat
)

REM Clean previous builds
echo Cleaning up previous builds...
if exist dist rmdir /s /q dist
mkdir dist

REM Build using CMake
echo Building with CMake...
if not exist build mkdir build
cd build

cmake .. -DCMAKE_TOOLCHAIN_FILE=C:/vcpkg/scripts/buildsystems/vcpkg.cmake -DCMAKE_BUILD_TYPE=Release

if %ERRORLEVEL% EQU 0 (
    cmake --build . --config Release
    if %ERRORLEVEL% EQU 0 (
        echo.
        echo ========================================
        echo Build successful!
        echo Executable created at dist\aitunes.exe
        echo ========================================
    ) else (
        echo Build failed!
        pause
        exit /b 1
    )
) else (
    echo CMake configuration failed!
    pause
    exit /b 1
)

cd ..

REM Alternative build using g++ directly
echo.
echo Alternative: Building with g++ directly...
g++ src\main.cpp -o dist\aitunes_gcc.exe ^
  -std=c++17 ^
  -I"C:\vcpkg\installed\x64-windows\include" ^
  -L"C:\vcpkg\installed\x64-windows\lib" ^
  -lcurl ^
  -lpdcurses ^
  -lwinmm ^
  -lws2_32 ^
  -static-libgcc ^
  -static-libstdc++

if %ERRORLEVEL% EQU 0 (
    echo Direct g++ build also successful!
    echo Executable created at dist\aitunes_gcc.exe
) else (
    echo Direct g++ build failed!
)

echo.
echo Build process completed!
pause 