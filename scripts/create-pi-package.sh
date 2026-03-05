#!/bin/bash
# Create Raspberry Pi deployment package (.tgz)
# For standard Raspberry Pi Linux (non-pCP) deployment

set -e

# Get target from argument or default to armv7
TARGET="${1:-armv7-unknown-linux-gnueabihf}"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ARCH=$(echo "${TARGET}" | cut -d'-' -f1)
PACKAGE_NAME="lymons-${VERSION}-pi-${ARCH}"
BUILD_DIR="/tmp/${PACKAGE_NAME}"
RUNTIME_DIR="usr/local/share/lymons"

echo "Creating Raspberry Pi deployment package..."
echo "Version: ${VERSION}"
echo "Target: ${TARGET}"
echo "Architecture: ${ARCH}"

# Verify cross-compiled binary exists
if [ ! -f "target/${TARGET}/release/LyMonS" ]; then
    echo "Error: Cross-compiled binary not found!"
    echo "Please run: ./scripts/cross-compile-pi.sh ${TARGET} first"
    exit 1
fi

# Clean and create build directory
rm -rf "${BUILD_DIR}"
mkdir -p "${BUILD_DIR}"/{usr/local/bin,${RUNTIME_DIR},${RUNTIME_DIR}/data,${RUNTIME_DIR}/fonts,${RUNTIME_DIR}/assets,${RUNTIME_DIR}/drivers,${RUNTIME_DIR}/config}

# Copy main binary
echo "Copying binary..."
cp "target/${TARGET}/release/LyMonS" "${BUILD_DIR}/usr/local/bin/"

# Strip binary if strip available
if command -v strip &> /dev/null; then
    strip "${BUILD_DIR}/usr/local/bin/LyMonS" 2>/dev/null || echo "Strip failed (non-fatal)"
fi

# Copy runtime assets and resources
echo "Copying runtime assets and resources..."
cp -fr "assets/" "${BUILD_DIR}/${RUNTIME_DIR}/"
cp -fr "data/" "${BUILD_DIR}/${RUNTIME_DIR}/"
cp -fr "fonts/" "${BUILD_DIR}/${RUNTIME_DIR}/"

# Copy plugins
echo "Copying plugins..."
if ls target/${TARGET}/release/drivers/liblymons_driver_*.so 1>/dev/null 2>&1; then
    cp target/${TARGET}/release/drivers/liblymons_driver_*.so "${BUILD_DIR}/${RUNTIME_DIR}/drivers/"
    if command -v strip &> /dev/null; then
        strip "${BUILD_DIR}/${RUNTIME_DIR}/drivers/"*.so 2>/dev/null || echo "Strip plugins failed (non-fatal)"
    fi
else
    echo "Warning: No plugin drivers found in target/${TARGET}/release/drivers/"
fi

# Copy configuration template (config lives in the application folder)
cat > "${BUILD_DIR}/${RUNTIME_DIR}/config/lymons.yaml.example" <<'EOF'
# LyMonS Configuration Example for Raspberry Pi

display:
  folder: /usr/local/share/lymons/drivers/
  driver: ssd1309          # Options: ssd1306, ssd1309, sh1106, ssd1322
  bus:
    type: i2c
    bus: "/dev/i2c-1"
    address: 0x3C          # Common: 0x3C or 0x3D
  brightness: 128          # 0-255
  rotate_deg: 0            # 0, 90, 180, 270
  invert: false

# location - your latitude/longitude - use https://www.latlong.net/
latitude: 42.36141    # change to your location
longitude: -71.10407  # change to your location

# Slim Server Connection (auto-detected if not specified)
# slimserver:
#   host: "localhost" # Or IP of your LMS server
#   port: 9000

# Player MAC Address (auto-detected if not specified)
# player_mac: "aa:bb:cc:dd:ee:ff"
EOF

# Create runtime convenience script
cat > "${BUILD_DIR}/${RUNTIME_DIR}/gomonitor" <<'EOF'
#!/bin/sh
# LyMonS for Raspberry Pi - it's worth the squeeze
BINDIR="/usr/local/share/lymons"

sudo killall LyMonS > /dev/null 2>&1
sudo killall LyMonS > /dev/null 2>&1

sudo modprobe i2c_dev > /dev/null 2>&1
sudo modprobe i2c-dev > /dev/null 2>&1

cd "${BINDIR}"
CMD="sudo LyMonS --config=${BINDIR}/config/lymons.yaml $@"
echo $CMD
eval $CMD > /dev/null &
exit
EOF
chmod +x "${BUILD_DIR}/${RUNTIME_DIR}/gomonitor"

# Create installation script
cat > "${BUILD_DIR}/install.sh" <<'EOF'
#!/bin/sh
# LyMonS Installation Script for Raspberry Pi
# Run as: sudo ./install.sh

echo "Installing LyMonS for Raspberry Pi..."
RUNTIME_DIR="/usr/local/share/lymons"

# Copy binary
cp -v usr/local/bin/LyMonS /usr/local/bin/
chmod +x /usr/local/bin/LyMonS

# Copy runtime folder (data, assets, fonts, drivers, gomonitor, config)
mkdir -p "${RUNTIME_DIR}"
cp -vfr ${RUNTIME_DIR}/. "${RUNTIME_DIR}/"
chmod +x "${RUNTIME_DIR}/gomonitor"

# Create config from example if not already present
if [ ! -f "${RUNTIME_DIR}/config/lymons.yaml" ]; then
    cp "${RUNTIME_DIR}/config/lymons.yaml.example" "${RUNTIME_DIR}/config/lymons.yaml"
    echo "Created default configuration at ${RUNTIME_DIR}/config/lymons.yaml"
    echo "Please edit this file with your settings"
fi

# piCorePlayer persistence (if applicable)
if [ -f /usr/local/sbin/filetool.sh ]; then
    grep -v "^${RUNTIME_DIR}" /opt/.filetool.lst > /tmp/.filetool.lst.tmp \
        && mv /tmp/.filetool.lst.tmp /opt/.filetool.lst
    echo "/usr/local/bin/LyMonS" >> /opt/.filetool.lst
    echo "${RUNTIME_DIR}" >> /opt/.filetool.lst
    echo "Added to piCorePlayer backup list"
    filetool.sh -b
fi

echo ""
echo "Installation complete!"
echo ""
echo "Next steps:"
echo "1. Edit ${RUNTIME_DIR}/config/lymons.yaml with your settings"
echo "2. Test: ${RUNTIME_DIR}/gomonitor"
echo "3. Add to autostart if desired"
EOF
chmod +x "${BUILD_DIR}/install.sh"

# Create package README
cat > "${BUILD_DIR}/README.md" <<EOF
# LyMonS ${VERSION} for Raspberry Pi (${ARCH})

Dynamic OLED display driver for Logitech Media Server.

**Architecture**: ${TARGET}

## Installation

1. Extract this package:
   \`\`\`bash
   tar xzf ${PACKAGE_NAME}.tgz
   cd ${PACKAGE_NAME}
   \`\`\`

2. Run the installation script:
   \`\`\`bash
   sudo ./install.sh
   \`\`\`

3. Edit configuration:
   \`\`\`bash
   sudo nano /usr/local/share/lymons/config/lymons.yaml
   \`\`\`

4. Test:
   \`\`\`bash
   /usr/local/share/lymons/gomonitor
   \`\`\`

## Supported Displays

- **SSD1306** - 128x64 I2C (most common)
- **SSD1309** - 128x64 I2C
- **SH1106**  - 132x64 I2C
- **SSD1322** - 256x64 SPI (grayscale)

## Files

- \`/usr/local/bin/LyMonS\` - Main binary
- \`/usr/local/share/lymons/gomonitor\` - Launch script
- \`/usr/local/share/lymons/drivers/\` - Plugin drivers
- \`/usr/local/share/lymons/data/\` - Runtime data
- \`/usr/local/share/lymons/assets/\` - Runtime assets
- \`/usr/local/share/lymons/fonts/\` - Fonts
- \`/usr/local/share/lymons/config/lymons.yaml.example\` - Configuration template
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

BINARY_SIZE=$(du -h "${BUILD_DIR}/usr/local/bin/LyMonS" | cut -f1)
PACKAGE_SIZE=$(du -h "${OLDPWD}/${PACKAGE_NAME}.tgz" | cut -f1)

echo ""
echo "✓ Package created: ${PACKAGE_NAME}.tgz"
echo "  Binary size: ${BINARY_SIZE}"
echo "  Package size: ${PACKAGE_SIZE}"
echo ""
echo "To install:"
echo "  tar xzf ${PACKAGE_NAME}.tgz"
echo "  cd ${PACKAGE_NAME}"
echo "  sudo ./install.sh"
