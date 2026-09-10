#!/bin/bash
# Cross-compile LyMonS for Raspberry Pi
#
# Pass the Rust target triple as the first argument.  Three are released:
#
#   arm-unknown-linux-gnueabihf      Pi 1 / Zero / Zero W        (ARMv6, 32-bit)
#   armv7-unknown-linux-gnueabihf    Pi 2 / 3 / 4 on a 32-bit OS (ARMv7)
#   aarch64-unknown-linux-gnu        Pi 3 / 4 / 5 / Zero 2 W on a 64-bit OS
#
# There is no armv6-* triple in Rust; the ARMv6 build uses arm-unknown-linux-
# gnueabihf and is labelled "armv6" in the packaging scripts and release assets.
#
# The armv7 binary also runs on the 32-bit Allwinner Orange Pi boards (Zero /
# Zero LTS on H2+/H3, PC, PC Plus, One).  Every 64-bit Orange Pi has its own
# script: ./scripts/cross-compile-opi.sh
#
# Every display driver is built regardless of target.  Bus wiring (I2C, SPI and
# the GPIO controller carrying DC/RST) is chosen at runtime from lymons.yaml,
# not at compile time.

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

# Build everything (main + plugins) using cross
echo "Building workspace (main binary + all plugins)..."
OPENSSL_STATIC=1 OPENSSL_VENDORED=1 cross build --release --workspace --target="${TARGET}"

# Organize plugins
echo "Organizing plugin binaries..."
mkdir -p "target/${TARGET}/release/drivers"
if ls target/${TARGET}/release/liblymons_driver_*.so 1>/dev/null 2>&1; then
    cp target/${TARGET}/release/liblymons_driver_*.so "target/${TARGET}/release/drivers/"
else
    echo "Warning: No plugin drivers found to organize"
fi

echo ""
echo "Cross-compilation complete!"
echo "Binary: target/${TARGET}/release/LyMonS"
echo "Plugins: target/${TARGET}/release/drivers/"
echo ""
echo "Binary size: $(du -h target/${TARGET}/release/LyMonS | cut -f1)"
echo "Total plugins size: $(du -sh target/${TARGET}/release/drivers/ | cut -f1)"
