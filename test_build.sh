#!/bin/bash

echo "Testing build process..."

# Make sure we're in the right directory
if [ ! -f "build.sh" ]; then
    echo "Error: build.sh not found. Please run this script from the project root."
    exit 1
fi

# Run the build script
echo "Running build script..."
chmod +x build.sh
./build.sh

# Check if the binary was created
if [ -f "dist/aitunes" ]; then
    echo "✅ Build successful! Binary created at dist/aitunes"
    
    # Check if the binary is executable
    if [ -x "dist/aitunes" ]; then
        echo "✅ Binary is executable"
    else
        echo "❌ Binary is not executable"
        exit 1
    fi
    
    # Check binary dependencies
    echo "Checking binary dependencies..."
    if command -v ldd >/dev/null 2>&1; then
        ldd dist/aitunes
    else
        echo "ldd not available, skipping dependency check"
    fi
    
else
    echo "❌ Build failed! Binary not found at dist/aitunes"
    exit 1
fi

echo "Build test completed successfully!" 