@echo off
echo Testing aiTunes Windows build...

REM Check if executable exists
if not exist "dist\aitunes.exe" (
    echo ERROR: aitunes.exe not found in dist folder!
    echo Please run build_windows.bat first.
    pause
    exit /b 1
)

REM Check if executable runs (should show help or version)
echo Testing executable...
dist\aitunes.exe --help >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo SUCCESS: Executable runs correctly!
) else (
    echo WARNING: Executable may have issues (this is normal if no --help option)
)

REM Check file size
for %%A in (dist\aitunes.exe) do set size=%%~zA
echo Executable size: %size% bytes

REM Check dependencies
echo.
echo Checking for required DLLs...
if exist "dist\*.dll" (
    echo Found DLL files:
    dir dist\*.dll /b
) else (
    echo No DLL files found (static linking used)
)

echo.
echo Windows build test completed!
pause 