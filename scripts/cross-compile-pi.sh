#!/bin/bash
# Cross-compile LyMonS for Raspberry Pi
# Supports Pi 3/4 (32-bit armv7) and Pi 4/5 (64-bit aarch64)

set -e

# Default to armv7 (32-bit) for maximum Pi compatibility
TARGET="${1:-armv7-unknown-linux-gnueabihf}"

echo "Cross-compiling LyMonS for Raspberry Pi..."
echo "Target: ${TARGET}"

# Install target if not already installed
rustup target add "${TARGET}" 2>/dev/null || true

# Check if cross is installed
if ! command -v cross &> /dev/null; then
    echo "Installing cross for easier cross-compilation..."
    cargo install cross --git https://github.com/cross-rs/cross
fi

# Build main binary using cross
echo "Building main binary..."
cross build --release --target="${TARGET}"

# Build plugins
echo "Building plugins..."
cd drivers/lymons-driver-ssd1306
cross build --release --target="${TARGET}"
cd ../..

cd drivers/lymons-driver-ssd1309
cross build --release --target="${TARGET}"
cd ../..

cd drivers/lymons-driver-sh1106
cross build --release --target="${TARGET}"
cd ../..

cd drivers/lymons-driver-ssd1322
cross build --release --target="${TARGET}"
cd ../..

# Organize plugins
echo "Organizing plugin binaries..."
mkdir -p "target/${TARGET}/release/drivers"
cp "target/${TARGET}/release/liblymons_driver_ssd1306.so" "target/${TARGET}/release/drivers/"
cp "target/${TARGET}/release/liblymons_driver_ssd1309.so" "target/${TARGET}/release/drivers/"
cp "target/${TARGET}/release/liblymons_driver_sh1106.so" "target/${TARGET}/release/drivers/"
cp "target/${TARGET}/release/liblymons_driver_ssd1322.so" "target/${TARGET}/release/drivers/"

echo ""
echo "Cross-compilation complete!"
echo "Binary: target/${TARGET}/release/LyMonS"
echo "Plugins: target/${TARGET}/release/drivers/"
echo ""
echo "Binary size: $(du -h target/${TARGET}/release/LyMonS | cut -f1)"
echo "Total plugins size: $(du -sh target/${TARGET}/release/drivers/ | cut -f1)"
