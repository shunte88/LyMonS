# Cross-Compilation for Raspberry Pi and Orange Pi

This document explains how to build LyMonS for Raspberry Pi and for the current
64-bit Orange Pi boards, Rockchip and Allwinner alike, using cross-compilation.

## Quick Start

### Build for Raspberry Pi (32-bit, most compatible)
```bash
make release_pi
```

This creates: `lymons-X.Y.Z-pi-armv7.tgz`

### Build for Raspberry Pi (64-bit, Pi 4/5)
```bash
make release_pi64
```

This creates: `lymons-X.Y.Z-pi-aarch64.tgz`

### Build for Raspberry Pi 1 / Zero / Zero W (armv6)
```bash
make release_pi_armv6
```

This creates: `lymons-X.Y.Z-pi-armv6.tgz`

### Build for Orange Pi (64-bit)
```bash
make release_opi
```

This creates: `lymons-X.Y.Z-opi-aarch64.tgz`

### A note on package names

The `make` targets above call `create-pi-package.sh`, which produces
`-pi-<arch>` packages that install into `/usr/local/share/lymons`. The GitHub
Actions release job instead calls `create-pcp-package.sh`, which produces the
`-pcp-<arch>` packages published on the `binaries` branch: same binaries and
same drivers, but with the universal installer that detects piCorePlayer and
TinyCore versus Raspberry Pi OS and deploys accordingly. Both ship
`show-buses.sh`.

## Prerequisites

### Local Development

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Install cross** (automatic on first `make release_pi`):
   ```bash
   cargo install cross --git https://github.com/cross-rs/cross
   ```

3. **Docker** (required by cross):
   - Install Docker: https://docs.docker.com/get-docker/
   - Ensure Docker daemon is running

### GitHub Actions

No setup required! Push to `main` branch or create a tag:
```bash
git tag -a v0.2.4 -m "Release v0.2.4"
git push origin v0.2.4
```

The workflow automatically builds armv6, armv7 and aarch64 for Raspberry Pi,
plus the aarch64 Orange Pi package.

## Architecture Support

| Target | Architecture | Raspberry Pi Models | Recommended |
|--------|-------------|---------------------|-------------|
| `arm-unknown-linux-gnueabihf` | 32-bit ARMv6 | Pi 1, Zero, Zero W | Only choice on ARMv6 |
| `armv7-unknown-linux-gnueabihf` | 32-bit ARMv7 | Pi 2, 3, 4, Zero 2 W | ✅ Most compatible |
| `aarch64-unknown-linux-gnu` | 64-bit ARM | Pi 3, 4, 5, 400, Zero 2 W | ⚡ Better performance |

**Recommendation**: Use `armv7` for maximum compatibility unless you specifically need 64-bit features.

Rust has no `armv6-*` target triple. The ARMv6 build uses
`arm-unknown-linux-gnueabihf`, and the packaging scripts and release assets
relabel it `armv6` so users are not handed an armv7 binary that will not run on
a Pi 1 or original Zero.

### Orange Pi

| Target | Architecture | Orange Pi Models | SoC | Package |
|--------|-------------|------------------|-----|---------|
| `aarch64-unknown-linux-gnu` | 64-bit ARM | 5 / 5B / 5 Plus / 5 Pro / 5 Max / CM5 | RK3588 / RK3588S | `-opi-aarch64` |
| `aarch64-unknown-linux-gnu` | 64-bit ARM | 3B | RK3566 | `-opi-aarch64` |
| `aarch64-unknown-linux-gnu` | 64-bit ARM | 4 / 4 LTS | RK3399 | `-opi-aarch64` |
| `aarch64-unknown-linux-gnu` | 64-bit ARM | Zero 3 / Zero 2W | Allwinner H618 | `-opi-aarch64` |
| `armv7-unknown-linux-gnueabihf` | 32-bit ARM | Zero / Zero LTS, PC, PC Plus, One | H2+ / H3 | `-pcp-armv7` |

One aarch64 package covers every current Orange Pi, Rockchip and Allwinner
alike. They differ only in GPIO line numbering, which is configuration. Only
the older 32-bit H2+/H3 boards fall outside it and take the Raspberry Pi
`armv7` package instead.

Orange Pi builds are a separate arm rather than a rename of the aarch64 Pi
package: the binary is the same, but the package ships an Armbian-only
installer, a `show-buses.sh` discovery helper, and an example config with
Orange Pi bus paths and GPIO bank numbering instead of Pi conventions.  See
[Orange Pi board setup](#orange-pi-board-setup) below.

## Manual Cross-Compilation

### Step 1: Cross-compile binaries
```bash
# 32-bit ARMv6 (Pi 1 / Zero / Zero W)
./scripts/cross-compile-pi.sh arm-unknown-linux-gnueabihf

# 32-bit ARMv7
./scripts/cross-compile-pi.sh armv7-unknown-linux-gnueabihf

# 64-bit (aarch64)
./scripts/cross-compile-pi.sh aarch64-unknown-linux-gnu
```

### Step 2: Create package
```bash
# 32-bit ARMv6, packaged as -pi-armv6
./scripts/create-pi-package.sh arm-unknown-linux-gnueabihf

# 32-bit ARMv7
./scripts/create-pi-package.sh armv7-unknown-linux-gnueabihf

# 64-bit
./scripts/create-pi-package.sh aarch64-unknown-linux-gnu
```

### Step 3: Verify package
```bash
tar tzf lymons-*-pi-armv7.tgz
```

### Orange Pi
```bash
./scripts/cross-compile-opi.sh aarch64-unknown-linux-gnu
./scripts/create-opi-package.sh aarch64-unknown-linux-gnu
tar tzf lymons-*-opi-aarch64.tgz
```

## Makefile Targets

| Target | Description |
|--------|-------------|
| `make cross_pi_armv6` | Cross-compile for armv6 (Pi 1 / Zero / Zero W) |
| `make release_pi_armv6` | Build complete package for armv6 |
| `make cross_pi` | Cross-compile for armv7 (32-bit) |
| `make cross_pi64` | Cross-compile for aarch64 (64-bit) |
| `make release_pi` | Build complete package for armv7 |
| `make release_pi64` | Build complete package for aarch64 |
| `make cross_opi` | Cross-compile for Orange Pi (aarch64, Rockchip + Allwinner) |
| `make release_opi` | Build complete package for Orange Pi |

## GitHub Actions Workflow

### Automatic Builds

The workflow triggers on:
- **Push to main/master**: Builds artifacts, uploads for 30 days
- **Tag push (v*)**: Builds artifacts AND creates GitHub release
- **Pull requests**: Validates cross-compilation works
- **Manual trigger**: Via GitHub Actions UI

### Workflow Jobs

1. **build-pi**: Cross-compiles for armv6, armv7 and aarch64
   - Caches cargo registry and build artifacts
   - Creates deployment packages
   - Uploads artifacts

2. **build-opi**: Cross-compiles for Orange Pi (aarch64, Rockchip + Allwinner)
   - Same binary as the Pi aarch64 build, packaged with the Armbian installer

3. **publish-binaries**: Pushes all packages to the `binaries` branch

4. **build-summary**: Reports overall build status

### Creating a Release

1. **Tag a release**:
   ```bash
   VERSION="0.2.4"
   git tag -a "v${VERSION}" -m "Release v${VERSION}"
   git push origin "v${VERSION}"
   ```

2. **Wait for workflow** (3-5 minutes)

3. **Check GitHub Releases**:
   - Navigate to: `https://github.com/YOUR_ORG/LyMonS/releases`
   - Download: `lymons-X.Y.Z-pcp-armv7.tgz`
   - Download: `lymons-X.Y.Z-pcp-aarch64.tgz`
   - Download: `lymons-X.Y.Z-opi-aarch64.tgz`

## Package Contents

Each `.tgz` file contains:

```
lymons-X.Y.Z-pcp-ARCH/
├── usr/local/bin/
│   └── LyMonS                           # Main binary
├── usr/local/lib/lymons/drivers/
│   ├── liblymons_driver_ssd1306.so      # Plugin: SSD1306 driver
│   ├── liblymons_driver_ssd1309.so      # Plugin: SSD1309 driver
│   ├── liblymons_driver_sh1106.so       # Plugin: SH1106 driver
│   └── liblymons_driver_ssd1322.so      # Plugin: SSD1322 driver
├── etc/lymons/
│   └── lymons.yaml.example              # Configuration template
├── install.sh                            # Installation script
└── README.md                             # Usage instructions
```

## Installation on Raspberry Pi

### 1. Copy package to Pi
```bash
scp lymons-*-pcp-armv7.tgz pi@raspberrypi.local:~
```

### 2. SSH to Pi
```bash
ssh pi@raspberrypi.local
```

### 3. Extract and install
```bash
tar xzf lymons-*-pcp-armv7.tgz
cd lymons-*-pcp-armv7
sudo ./install.sh
```

### 4. Configure
```bash
sudo nano /etc/lymons/lymons.yaml
```

### 5. Test
```bash
LyMonS
```

## Orange Pi board setup

Orange Pi has no `raspi-config` and no `/boot/config.txt`.  Enable the bus you
need in `/boot/armbianEnv.txt` (`armbian-config` → System → Hardware lists the
overlays your board supports), then reboot:

```
overlays=i2c5-m3 spi4-m0-cs1-spidev
```

Every package ships `show-buses.sh`, which lists the resulting `/dev/i2c-*`,
`/dev/spidev*` and GPIO controllers. Run it before editing `lymons.yaml`.

### GPIO pins are not BCM numbers

On a Raspberry Pi the whole 40-pin header is one GPIO controller whose cdev line
offsets happen to equal the BCM numbers, so `dc_pin: 24` means BCM 24 and no
chip needs naming.

**Rockchip** (OPi 5 family, 3B, 4) registers one controller per bank, `gpio0`
through `gpio4` with 32 lines each, so there is no single header chip and no
global numbering. Name the bank with `gpio_chip` and give a bank-relative line:

```
line = group * 8 + index        (group A=0, B=1, C=2, D=3)
PC7  = 2 * 8 + 7 = 23           on bank 3
```

**Allwinner** (OPi Zero 3, Zero 2W) has two controllers, the main pinctrl and
the R_PIO carrying the PL bank, each numbered flat across its banks:

```
line = bank * 32 + index        (bank A=0, B=1, C=2 ... I=8)
PC7  = 2 * 32 + 7 = 71          on the main pinctrl
PL10 = 10                       on the R_PIO controller
```

Controller labels are device-tree node names (`gpio3` on Rockchip,
`300b000.pinctrl` on H618) and vary by SoC, so read them off the board with
`show-buses.sh` rather than assuming.

```yaml
display:
  bus:
    type: spi
    bus: "/dev/spidev4.0"
    gpio_chip: "gpio3"    # bank label; also accepts "/dev/gpiochip3" or "3"
    dc_pin: 23            # bank-relative line, NOT a BCM number
    rst_pin: 24
```

`gpio_chip` is optional and ignored on a Pi, where the header controller is
still detected by label (`pinctrl-rp1`, `pinctrl-bcm2711` and so on). If nothing
matches, LyMonS falls back to `/dev/gpiochip0` and logs every controller the
kernel exposes so you can see what to configure.

Plugin drivers are loaded as shared objects and never see the YAML config, so
set `LYMONS_GPIO_CHIP=gpio3` in the environment for those.

## Troubleshooting

### cross not found
```bash
cargo install cross --git https://github.com/cross-rs/cross
```

### Docker permission denied
```bash
sudo usermod -aG docker $USER
# Log out and back in
```

### Binary won't run on Pi
- Check architecture: `uname -m` on Pi
  - `armv7l` → Use `armv7` package
  - `aarch64` → Use `aarch64` package
- Verify target matches Pi OS (32-bit vs 64-bit)

### Binary won't run on Orange Pi
- `uname -m` should report `aarch64` for the RK3588/RK3566/RK3399 boards
- A `GLIBC_x.y not found` error means the Armbian image is older than the
  `cross` build image; check `ldd --version` on both and either update Armbian
  or build on a host matching its glibc

### Display never appears on Orange Pi
- Run `show-buses.sh`. An empty `/dev/i2c-*` or `/dev/spidev*` means the
  overlay is not enabled in `/boot/armbianEnv.txt`
- For SPI, check the log for `No Raspberry Pi header GPIO controller found`;
  that means `gpio_chip` is unset and the fallback `/dev/gpiochip0` (bank 0) is
  almost certainly the wrong bank

### Missing dependencies on Pi
```bash
# PiCorePlayer/TinyCore
tce-load -wi i2c-tools
tce-load -wi python3.9

# Raspberry Pi OS
sudo apt update
sudo apt install i2c-tools
```

## CI/CD Pipeline

### Workflow File Location
`.github/workflows/release-pi.yml`

### Customization

**Change trigger branches**:
```yaml
on:
  push:
    branches:
      - main
      - develop  # Add more branches
```

**Add more architectures**:
```yaml
strategy:
  matrix:
    target:
      - armv7-unknown-linux-gnueabihf
      - aarch64-unknown-linux-gnu
      - x86_64-unknown-linux-gnu  # Add x86_64
```

**Adjust cache retention**:
```yaml
- name: Upload build artifact
  uses: actions/upload-artifact@v4
  with:
    retention-days: 90  # Keep for 90 days
```

## Performance

### Build Times (GitHub Actions)

| Target | Compile Time | Package Size |
|--------|-------------|--------------|
| armv7 | ~5 minutes | ~3.5 MB |
| aarch64 | ~5 minutes | ~3.8 MB |

### Caching

The workflow caches:
- Cargo registry (~500 MB)
- Cargo index (~100 MB)
- Build artifacts (~2 GB)

First build: ~8 minutes
Subsequent builds: ~3 minutes

## Support

For issues:
1. Check workflow logs in GitHub Actions
2. Verify `make release_pi` works locally
3. Test package on actual Pi hardware
4. Open GitHub issue with logs

## License

GPL-3.0-or-later
