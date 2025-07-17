cmake_minimum_required(VERSION 3.15)
project(aitunes VERSION 1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXE_LINKER_FLAGS "-static -static-libgcc -static-libstdc++")

# Include nlohmann/json (header-only)
include_directories(${CMAKE_SOURCE_DIR}/deps/json)

# Manually specify paths for static libs
include_directories(${CMAKE_SOURCE_DIR}/libs/x86_64/static/include)
link_directories(${CMAKE_SOURCE_DIR}/libs/x86_64/static/lib)

# Define the executable
add_executable(aitunes src/main.cpp)

# Link statically against required libs
target_link_libraries(aitunes
  -Wl,--whole-archive
  curl ncursesw vlc
  -Wl,--no-whole-archive
)
