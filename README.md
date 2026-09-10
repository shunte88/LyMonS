# LyMonS Binaries

Pre-compiled binaries for Raspberry Pi (TinyCore Linux / PiCorePlayer)
and for the current Rockchip Orange Pi boards (Armbian / Debian).

## Download Latest — Raspberry Pi

- **32-bit (armv6)** - Raspberry Pi 1, 2, 3, Zero: [lymons-latest-pcp-armv6.tgz](latest/lymons-latest-pcp-armv6.tgz)
- **32-bit (armv7)** - Raspberry Pi 3, 4, Zero 2 W: [lymons-latest-pcp-armv7.tgz](latest/lymons-latest-pcp-armv7.tgz)
- **64-bit (aarch64)** - Raspberry Pi 4, 5, 400: [lymons-latest-pcp-aarch64.tgz](latest/lymons-latest-pcp-aarch64.tgz)

## Download Latest — Orange Pi

- **64-bit (aarch64)** - Orange Pi 5 family (RK3588), 3B (RK3566), 4 (RK3399): [lymons-latest-opi-aarch64.tgz](latest/lymons-latest-opi-aarch64.tgz)

Older 32-bit Allwinner Orange Pis (Zero, PC, One) use the `armv7` package above.

## Installation

1. Download the appropriate package for your Pi
2. Extract: `tar xzf lymons-latest-pcp-*.tgz`
3. Install: `cd lymons-*-pcp-* && sudo ./install.sh`
4. Configure: `sudo nano /etc/lymons/lymons.yaml`

## Versioned Builds

See the `armv6/`, `armv7/`, `aarch64/` and `opi-aarch64/` directories for specific versions.

---
*Built automatically by GitHub Actions*
