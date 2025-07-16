echo "Cleaning up previous builds..."
rm -rf dist
mkdir -p dist
echo "Compiling for linux..."
g++ main.cpp -o dist/caitunes \
  -std=c++17 \
  $(pkg-config --cflags --libs libcurl) \
  -lncurses \
  -lvlc
echo "Done"
# echo ""
# echo "Compiling for windows..."

# x86_64-w64-mingw32-g++ -std=c++17 -O2 \
#   -Ivlc-sdk/include \
#   main.cpp \
#   -Lvlc-sdk/lib/mingw \
#   -lvlc.x64.dll \
#   -lvlccore.x64.dll \
#   $(x86_64-w64-mingw32-pkg-config --cflags --libs libcurl) \
#   -lpdcurses \
#   -o dist/caitunes.exe

# echo "Done"
