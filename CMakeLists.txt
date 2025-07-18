cmake_minimum_required(VERSION 3.16)
project(aitunes VERSION 2.0)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Find required packages
find_package(CURL REQUIRED)
find_package(PDCurses REQUIRED)
find_package(nlohmann_json REQUIRED)

# Platform-specific settings
if(WIN32)
    # Windows-specific libraries
    set(PLATFORM_LIBS winmm ws2_32)
    set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -static-libgcc -static-libstdc++")
else()
    # Linux-specific libraries
    set(PLATFORM_LIBS pthread asound)
endif()

# Create executable
add_executable(aitunes src/main.cpp)

# Include directories
target_include_directories(aitunes PRIVATE include)

# Link libraries
target_link_libraries(aitunes PRIVATE 
    CURL::libcurl 
    PDCurses::pdcurses 
    nlohmann_json::nlohmann_json
    ${PLATFORM_LIBS}
)

# Set output directory
set_target_properties(aitunes PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/dist
)

# Copy dependencies to output directory if needed
if(WIN32)
    add_custom_command(TARGET aitunes POST_BUILD
        COMMAND ${CMAKE_COMMAND} -E copy_if_different
        $<TARGET_FILE:aitunes>
        ${CMAKE_BINARY_DIR}/dist/aitunes.exe
    )
endif() 