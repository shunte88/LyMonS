#!/bin/bash
# Cross-compile LyMonS for Orange Pi (current 64-bit generation)
#
# Covers the current 64-bit Orange Pi boards:
#   Orange Pi 5 / 5B / 5 Plus / 5 Pro / 5 Max / CM5   (Rockchip RK3588 / RK3588S)
#   Orange Pi 3B                                       (Rockchip RK3566)
#   Orange Pi 4 / 4 LTS                                (Rockchip RK3399)
#   Orange Pi Zero 3 / Zero 2W                         (Allwinner H618)
#
# All of these are aarch64, so there is a single target.  Only the older 32-bit
# Allwinner boards (Zero / Zero LTS on H2+/H3, PC, PC Plus, One) fall outside it
# — build those with ./scripts/cross-compile-pi.sh armv7-unknown-linux-gnueabihf,
# which produces a compatible armv7 binary.

set -e

TARGET="${1:-aarch64-unknown-linux-gnu}"

case "${TARGET}" in
    aarch64-*) ;;
    *)
        echo "Error: Orange Pi builds target aarch64 (got '${TARGET}')."
        echo "For 32-bit Allwinner boards use ./scripts/cross-compile-pi.sh armv7-unknown-linux-gnueabihf"
        exit 1
        ;;
esac

echo "Cross-compiling LyMonS for Orange Pi (aarch64)..."
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
