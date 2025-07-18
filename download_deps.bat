@echo off
echo Downloading dr_mp3 and miniaudio headers...

REM Create include directory if it doesn't exist
if not exist include mkdir include

REM Download dr_mp3
echo Downloading dr_mp3...
powershell -Command "Invoke-WebRequest -Uri 'https://raw.githubusercontent.com/mackron/dr_libs/master/dr_mp3.h' -OutFile 'include\dr_mp3.h'"

REM Download miniaudio
echo Downloading miniaudio...
powershell -Command "Invoke-WebRequest -Uri 'https://raw.githubusercontent.com/mackron/miniaudio/master/miniaudio.h' -OutFile 'include\miniaudio.h'"

echo Dependencies downloaded successfully! 