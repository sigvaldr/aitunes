echo "Cleaning up previous builds..."
rm -rf dist
mkdir -p dist

# Download dependencies if they don't exist
if [ ! -f "include/dr_mp3.h" ] || [ ! -f "include/miniaudio.h" ]; then
    echo "Downloading dependencies..."
    chmod +x download_deps.sh
    ./download_deps.sh
fi

echo "Compiling for linux..."
g++ src/main.cpp -o dist/aitunes \
  -std=c++17 \
  $(pkg-config --cflags --libs libcurl) \
  -lncurses \
  -lpthread \
  -lasound
echo "Done"
