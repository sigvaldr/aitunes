rm caitunes

rm main.cpp

wl-paste >main.cpp

g++ main.cpp -o caitunes \
  -std=c++17 \
  $(pkg-config --cflags --libs libcurl) \
  -lncurses \
  -lvlc
