#!/bin/bash
# Create PiCorePlayer deployment package (.tgz)
# For TinyCore Linux deployment on PiCorePlayer

set -e

VERSION="0.2.1"
PACKAGE_NAME="lymons-${VERSION}-pcp"
BUILD_DIR="/tmp/${PACKAGE_NAME}"

echo "Creating PiCorePlayer deployment package..."

# Clean and create build directory
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"/{usr/local/bin,usr/local/lib/lymons/drivers,usr/local/share/lymons,etc/lymons}

# Build LyMonS binary
echo "Building LyMonS binary..."
cargo build --release

# Copy main binary
cp target/release/LyMonS "${BUILD_DIR}/usr/local/bin/"
strip "${BUILD_DIR}/usr/local/bin/LyMonS"

# Build and copy plugins
echo "Building plugins..."
make plugins

cp target/release/drivers/liblymons_driver_ssd1306.so "${BUILD_DIR}/usr/local/lib/lymons/drivers/"
cp target/release/drivers/liblymons_driver_ssd1309.so "${BUILD_DIR}/usr/local/lib/lymons/drivers/"
cp target/release/drivers/liblymons_driver_sh1106.so "${BUILD_DIR}/usr/local/lib/lymons/drivers/"
cp target/release/drivers/liblymons_driver_ssd1322.so "${BUILD_DIR}/usr/local/lib/lymons/drivers/"

strip "${BUILD_DIR}"/usr/local/lib/lymons/drivers/*.so

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
# LyMonS ${VERSION} for PiCorePlayer

Dynamic plugin-based OLED display driver for Logitech Media Server players.

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

## Package Size

- Binary: ~8.9 MB (with all features)
- Plugins: ~1.4 MB (all 4 drivers)
- Total: ~10.3 MB

## License

GPL-3.0-or-later

## Version

${VERSION} ($(date +%Y-%m-%d))
EOF

# Create package
cd /tmp
tar czf "${PACKAGE_NAME}.tgz" "${PACKAGE_NAME}/"

# Move to project root
mv "${PACKAGE_NAME}.tgz" "/data2/refactor/LyMonS/"

echo ""
echo "Package created: ${PACKAGE_NAME}.tgz"
echo "Size: $(du -h /data2/refactor/LyMonS/${PACKAGE_NAME}.tgz | cut -f1)"
echo ""
echo "To test:"
echo "  tar xzf ${PACKAGE_NAME}.tgz"
echo "  cd ${PACKAGE_NAME}"
echo "  sudo ./install.sh"
