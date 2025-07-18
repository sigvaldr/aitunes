#!/bin/bash

echo "Downloading dr_mp3 and miniaudio headers..."

# Create include directory if it doesn't exist
mkdir -p include

# Download dr_mp3
echo "Downloading dr_mp3..."
curl -L -o include/dr_mp3.h https://raw.githubusercontent.com/mackron/dr_libs/master/dr_mp3.h

# Download miniaudio
echo "Downloading miniaudio..."
curl -L -o include/miniaudio.h https://raw.githubusercontent.com/mackron/miniaudio/master/miniaudio.h

echo "Dependencies downloaded successfully!" 