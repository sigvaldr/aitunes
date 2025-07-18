@echo off
echo Cleaning up previous builds...
if exist dist rmdir /s /q dist
mkdir dist

REM Download dependencies if they don't exist
if not exist "include\dr_mp3.h" (
    echo Downloading dependencies...
    call download_deps.bat
)

echo Compiling for Windows...
g++ src\main.cpp -o dist\aitunes.exe ^
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
    echo Build successful! Executable created at dist\aitunes.exe
) else (
    echo Build failed!
    pause
) 