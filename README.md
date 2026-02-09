# LyMonS Raspberry Pi Binaries

Pre-compiled binaries for Raspberry Pi (TinyCore Linux / PiCorePlayer).

## Download Latest

- **32-bit (armv7)** - Raspberry Pi 3, 4, Zero 2 W: [lymons-latest-pcp-armv7.tgz](latest/lymons-latest-pcp-armv7.tgz)
- **64-bit (aarch64)** - Raspberry Pi 4, 5, 400: [lymons-latest-pcp-aarch64.tgz](latest/lymons-latest-pcp-aarch64.tgz)

## Installation

1. Download the appropriate package for your Pi
2. Extract: `tar xzf lymons-latest-pcp-*.tgz`
3. Install: `cd lymons-*-pcp-* && sudo ./install.sh`
4. Configure: `sudo nano /etc/lymons/lymons.yaml`

## Versioned Builds

See the `armv7/` and `aarch64/` directories for specific versions.

---
*Built automatically by GitHub Actions*
