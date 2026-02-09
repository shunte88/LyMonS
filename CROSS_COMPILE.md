# Cross-Compilation for Raspberry Pi

This document explains how to build LyMonS for Raspberry Pi using cross-compilation.

## Quick Start

### Build for Raspberry Pi (32-bit, most compatible)
```bash
make release_pi
```

This creates: `lymons-X.Y.Z-pcp-armv7.tgz`

### Build for Raspberry Pi (64-bit, Pi 4/5)
```bash
make release_pi64
```

This creates: `lymons-X.Y.Z-pcp-aarch64.tgz`

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

The workflow automatically builds for both armv7 and aarch64.

## Architecture Support

| Target | Architecture | Raspberry Pi Models | Recommended |
|--------|-------------|---------------------|-------------|
| `armv7-unknown-linux-gnueabihf` | 32-bit ARM | Pi 2, 3, 4, Zero 2 W | ✅ Most compatible |
| `aarch64-unknown-linux-gnu` | 64-bit ARM | Pi 4, 5, 400 | ⚡ Better performance |

**Recommendation**: Use `armv7` for maximum compatibility unless you specifically need 64-bit features.

## Manual Cross-Compilation

### Step 1: Cross-compile binaries
```bash
# 32-bit (armv7)
./scripts/cross-compile-pi.sh armv7-unknown-linux-gnueabihf

# 64-bit (aarch64)
./scripts/cross-compile-pi.sh aarch64-unknown-linux-gnu
```

### Step 2: Create package
```bash
# 32-bit
./scripts/create-pi-package.sh armv7-unknown-linux-gnueabihf

# 64-bit
./scripts/create-pi-package.sh aarch64-unknown-linux-gnu
```

### Step 3: Verify package
```bash
tar tzf lymons-*-pcp-armv7.tgz
```

## Makefile Targets

| Target | Description |
|--------|-------------|
| `make cross_pi` | Cross-compile for armv7 (32-bit) |
| `make cross_pi64` | Cross-compile for aarch64 (64-bit) |
| `make release_pi` | Build complete package for armv7 |
| `make release_pi64` | Build complete package for aarch64 |

## GitHub Actions Workflow

### Automatic Builds

The workflow triggers on:
- **Push to main/master**: Builds artifacts, uploads for 30 days
- **Tag push (v*)**: Builds artifacts AND creates GitHub release
- **Pull requests**: Validates cross-compilation works
- **Manual trigger**: Via GitHub Actions UI

### Workflow Jobs

1. **build-pi**: Cross-compiles for both armv7 and aarch64
   - Caches cargo registry and build artifacts
   - Creates deployment packages
   - Uploads artifacts

2. **build-summary**: Reports overall build status

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
