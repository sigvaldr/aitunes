# aiTunes Windows Build Script (PowerShell)
# Run this script in PowerShell with execution policy: Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser

Write-Host "========================================" -ForegroundColor Green
Write-Host "aiTunes Windows Build Script" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green

# Check if vcpkg is installed
if (-not (Test-Path "C:\vcpkg\vcpkg.exe")) {
    Write-Host "vcpkg not found. Installing vcpkg..." -ForegroundColor Yellow
    git clone https://github.com/Microsoft/vcpkg.git C:\vcpkg
    & C:\vcpkg\bootstrap-vcpkg.bat
    & C:\vcpkg\vcpkg integrate install
}

# Install required packages
Write-Host "Installing required packages..." -ForegroundColor Yellow
& C:\vcpkg\vcpkg install curl:x64-windows
& C:\vcpkg\vcpkg install pdcurses:x64-windows
& C:\vcpkg\vcpkg install nlohmann-json:x64-windows

# Download dependencies if they don't exist
if (-not (Test-Path "include\dr_mp3.h")) {
    Write-Host "Downloading dependencies..." -ForegroundColor Yellow
    & .\download_deps.bat
}

# Clean previous builds
Write-Host "Cleaning up previous builds..." -ForegroundColor Yellow
if (Test-Path "dist") { Remove-Item -Recurse -Force "dist" }
New-Item -ItemType Directory -Path "dist" | Out-Null

# Build using CMake
Write-Host "Building with CMake..." -ForegroundColor Yellow
if (-not (Test-Path "build")) { New-Item -ItemType Directory -Path "build" | Out-Null }
Set-Location build

$cmakeResult = & cmake .. -DCMAKE_TOOLCHAIN_FILE=C:/vcpkg/scripts/buildsystems/vcpkg.cmake -DCMAKE_BUILD_TYPE=Release
if ($LASTEXITCODE -eq 0) {
    $buildResult = & cmake --build . --config Release
    if ($LASTEXITCODE -eq 0) {
        Write-Host "" -ForegroundColor Green
        Write-Host "========================================" -ForegroundColor Green
        Write-Host "Build successful!" -ForegroundColor Green
        Write-Host "Executable created at dist\aitunes.exe" -ForegroundColor Green
        Write-Host "========================================" -ForegroundColor Green
    } else {
        Write-Host "Build failed!" -ForegroundColor Red
        Read-Host "Press Enter to continue"
        exit 1
    }
} else {
    Write-Host "CMake configuration failed!" -ForegroundColor Red
    Read-Host "Press Enter to continue"
    exit 1
}

Set-Location ..

# Alternative build using g++ directly
Write-Host "" -ForegroundColor Yellow
Write-Host "Alternative: Building with g++ directly..." -ForegroundColor Yellow
$gppResult = & g++ src\main.cpp -o dist\aitunes_gcc.exe -std=c++17 -I"C:\vcpkg\installed\x64-windows\include" -L"C:\vcpkg\installed\x64-windows\lib" -lcurl -lpdcurses -lwinmm -lws2_32 -static-libgcc -static-libstdc++

if ($LASTEXITCODE -eq 0) {
    Write-Host "Direct g++ build also successful!" -ForegroundColor Green
    Write-Host "Executable created at dist\aitunes_gcc.exe" -ForegroundColor Green
} else {
    Write-Host "Direct g++ build failed!" -ForegroundColor Red
}

Write-Host "" -ForegroundColor Green
Write-Host "Build process completed!" -ForegroundColor Green
Read-Host "Press Enter to continue" 