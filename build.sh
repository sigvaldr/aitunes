echo "Cleaning up previous builds..."
rm -rf dist
mkdir -p dist

# Download dependencies if they don't exist
if [ ! -f "include/dr_mp3.h" ] || [ ! -f "include/miniaudio.h" ]; then
    echo "Downloading dependencies..."
    chmod +x download_deps.sh
    ./download_deps.sh
fi

# Build PDCurses X11 if not already built
if [ ! -f "vendor/PDCurses/x11/libXCurses.a" ]; then
    echo "Building PDCurses (X11 backend)..."
    (cd vendor/PDCurses/x11 && ./configure --disable-shared && make)
fi

echo "Compiling for linux..."
g++ src/main.cpp -o dist/aitunes \
  -std=c++17 \
  $(pkg-config --cflags --libs libcurl) \
  -Ivendor/PDCurses \
  vendor/PDCurses/x11/libXCurses.a \
  -lpthread \
  -lasound \
  -lX11 -lXt -lXaw -lXmu -lXpm -lSM -lICE -lXext 
echo "Done"
