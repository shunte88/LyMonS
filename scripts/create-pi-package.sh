#!/bin/bash
# Create Raspberry Pi deployment package (.tgz)
# For TinyCore Linux deployment on PiCorePlayer

set -e

# Get target from argument or default to armv7
TARGET="${1:-armv7-unknown-linux-gnueabihf}"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ARCH=$(echo "${TARGET}" | cut -d'-' -f1)
PACKAGE_NAME="lymons-${VERSION}-pcp-${ARCH}"
BUILD_DIR="/tmp/${PACKAGE_NAME}"

echo "Creating Raspberry Pi deployment package..."
echo "Version: ${VERSION}"
echo "Target: ${TARGET}"
echo "Architecture: ${ARCH}"

# Verify cross-compiled binaries exist
if [ ! -f "target/${TARGET}/release/LyMonS" ]; then
    echo "Error: Cross-compiled binary not found!"
    echo "Please run: make cross_pi or ./scripts/cross-compile-pi.sh first"
    exit 1
fi

# Clean and create build directory
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"/{usr/local/bin,usr/local/lib/lymons/drivers,usr/local/share/lymons,etc/lymons}

# Copy main binary
echo "Copying binaries..."
cp "target/${TARGET}/release/LyMonS" "${BUILD_DIR}/usr/local/bin/"

# Strip binary if strip available
if command -v strip &> /dev/null; then
    strip "${BUILD_DIR}/usr/local/bin/LyMonS" 2>/dev/null || echo "Strip failed (non-fatal)"
fi

# Copy plugins
cp "target/${TARGET}/release/drivers/liblymons_driver_ssd1306.so" "${BUILD_DIR}/usr/local/lib/lymons/drivers/"
cp "target/${TARGET}/release/drivers/liblymons_driver_ssd1309.so" "${BUILD_DIR}/usr/local/lib/lymons/drivers/"
cp "target/${TARGET}/release/drivers/liblymons_driver_sh1106.so" "${BUILD_DIR}/usr/local/lib/lymons/drivers/"
cp "target/${TARGET}/release/drivers/liblymons_driver_ssd1322.so" "${BUILD_DIR}/usr/local/lib/lymons/drivers/"

# Strip plugins if strip available
if command -v strip &> /dev/null; then
    strip "${BUILD_DIR}"/usr/local/lib/lymons/drivers/*.so 2>/dev/null || echo "Strip plugins failed (non-fatal)"
fi

# Copy configuration templates
cat > "${BUILD_DIR}/etc/lymons/lymons.yaml.example" <<'EOF'
# LyMonS Configuration Example for PiCorePlayer

display:
  driver: ssd1306          # Options: ssd1306, ssd1309, sh1106, ssd1322
  bus:
    type: i2c
    bus: "/dev/i2c-1"
    address: 0x3C          # Common: 0x3C or 0x3D
  brightness: 128          # 0-255
  rotate_deg: 0            # 0, 90, 180, 270
  invert: false

# Slim Server Connection
slimserver:
  host: "localhost"        # Or IP of your LMS server
  port: 9000

# Player MAC Address (auto-detected if not specified)
# player_mac: "aa:bb:cc:dd:ee:ff"
EOF

# Create installation script
cat > "${BUILD_DIR}/install.sh" <<'EOF'
#!/bin/sh
# LyMonS Installation Script for PiCorePlayer
# Run as: sudo ./install.sh

echo "Installing LyMonS for PiCorePlayer..."

# Copy files
cp -v usr/local/bin/LyMonS /usr/local/bin/
chmod +x /usr/local/bin/LyMonS

mkdir -p /usr/local/lib/lymons/drivers
cp -v usr/local/lib/lymons/drivers/*.so /usr/local/lib/lymons/drivers/

mkdir -p /etc/lymons
if [ ! -f /etc/lymons/lymons.yaml ]; then
    cp -v etc/lymons/lymons.yaml.example /etc/lymons/lymons.yaml
    echo "Created default configuration at /etc/lymons/lymons.yaml"
    echo "Please edit this file with your settings"
fi

# Make persistent on PiCorePlayer
if [ -f /usr/local/sbin/filetool.sh ]; then
    echo "/usr/local/bin/LyMonS" >> /opt/.filetool.lst
    echo "/usr/local/lib/lymons" >> /opt/.filetool.lst
    echo "/etc/lymons" >> /opt/.filetool.lst
    echo "Added to PiCorePlayer backup list"

    # Backup
    filetool.sh -b
    echo "Backup completed"
fi

echo "Installation complete!"
echo ""
echo "Next steps:"
echo "1. Edit /etc/lymons/lymons.yaml with your configuration"
echo "2. Run: LyMonS to test"
echo "3. Add to autostart if desired"
EOF

chmod +x "${BUILD_DIR}/install.sh"

# Create README
cat > "${BUILD_DIR}/README.md" <<EOF
# LyMonS ${VERSION} for PiCorePlayer (${ARCH})

Dynamic plugin-based OLED display driver for Logitech Media Server players.

**Architecture**: ${TARGET}
**Built for**: Raspberry Pi (TinyCore Linux / PiCorePlayer)

## Installation

1. Extract this package:
   \`\`\`bash
   tar xzf ${PACKAGE_NAME}.tgz
   cd ${PACKAGE_NAME}
   \`\`\`

2. Run installation script:
   \`\`\`bash
   sudo ./install.sh
   \`\`\`

3. Edit configuration:
   \`\`\`bash
   sudo nano /etc/lymons/lymons.yaml
   \`\`\`

4. Test:
   \`\`\`bash
   LyMonS
   \`\`\`

## Supported Displays

- **SSD1306** - 128x64 I2C (most common)
- **SSD1309** - 128x64 I2C
- **SH1106** - 132x64 I2C
- **SSD1322** - 256x64 SPI (grayscale)

## Plugin System

Drivers are loaded dynamically from:
- \`/usr/local/lib/lymons/drivers/\` (system-wide)
- \`~/.local/lib/lymons/drivers/\` (user-local)

Plugins are automatically discovered and loaded based on your \`driver\` setting in the config.

## Configuration

Example I2C configuration (SSD1306):
\`\`\`yaml
display:
  driver: ssd1306
  bus:
    type: i2c
    bus: "/dev/i2c-1"
    address: 0x3C
  brightness: 128
\`\`\`

Example SPI configuration (SSD1322):
\`\`\`yaml
display:
  driver: ssd1322
  bus:
    type: spi
    bus: "/dev/spidev0.0"
    dc_pin: 24
    rst_pin: 25
\`\`\`

## Autostart on PiCorePlayer

Add to \`/opt/bootlocal.sh\`:
\`\`\`bash
/usr/local/bin/LyMonS &
\`\`\`

## Files Included

- \`usr/local/bin/LyMonS\` - Main binary
- \`usr/local/lib/lymons/drivers/*.so\` - Plugin drivers (4 total)
- \`etc/lymons/lymons.yaml.example\` - Configuration template
- \`install.sh\` - Installation script

## License

GPL-3.0-or-later

## Version

${VERSION} ($(date +%Y-%m-%d))
Built for: ${TARGET}
EOF

# Create package
cd /tmp
echo "Creating tarball..."
tar czf "${PACKAGE_NAME}.tgz" "${PACKAGE_NAME}/"

# Move to project root
mv "${PACKAGE_NAME}.tgz" "${OLDPWD}/"

# Calculate sizes
BINARY_SIZE=$(du -h "${BUILD_DIR}/usr/local/bin/LyMonS" | cut -f1)
PLUGINS_SIZE=$(du -sh "${BUILD_DIR}/usr/local/lib/lymons/drivers/" | cut -f1)
PACKAGE_SIZE=$(du -h "${OLDPWD}/${PACKAGE_NAME}.tgz" | cut -f1)

echo ""
echo "✓ Package created: ${PACKAGE_NAME}.tgz"
echo "  Binary size: ${BINARY_SIZE}"
echo "  Plugins size: ${PLUGINS_SIZE}"
echo "  Package size: ${PACKAGE_SIZE}"
echo ""
echo "To test:"
echo "  tar xzf ${PACKAGE_NAME}.tgz"
echo "  cd ${PACKAGE_NAME}"
echo "  sudo ./install.sh"
