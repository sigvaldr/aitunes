echo "Cleaning up previous builds..."
rm -rf dist
mkdir -p dist
echo "Compiling for linux..."
g++ src/main.cpp -o dist/aitunes \
  -std=c++17 \
  $(pkg-config --cflags --libs libcurl) \
  -lncurses \
  -lvlc
echo "Done"
