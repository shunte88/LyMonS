#!/bin/bash
# Create piCorePlayer deployment package (.tgz)
# For TinyCore Linux deployment on piCorePlayer

set -e

TARGET="${1:-armv7-unknown-linux-gnueabihf}"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ARCH=$(echo "${TARGET}" | cut -d'-' -f1)
PACKAGE_NAME="lymons-${VERSION}-pcp-${ARCH}"
BUILD_DIR="/tmp/${PACKAGE_NAME}"
# tinycore is a read only OS that does not survive across reboots
# all deliverables must be written to the mount, and added to the
# automated backup for inclusion in the persistence mechanism
RUNTIME_DIR="mnt/mmcblk0p2/tce/lymons"

echo "Creating piCorePlayer deployment package..."
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
mkdir -p "${BUILD_DIR}"/{${RUNTIME_DIR},${RUNTIME_DIR}/data,${RUNTIME_DIR}/assets,${RUNTIME_DIR}/fonts,${RUNTIME_DIR}/drivers,${RUNTIME_DIR}/config}

# Copy main binary
echo "Copying binary..."
cp "target/${TARGET}/release/LyMonS" "${BUILD_DIR}/${RUNTIME_DIR}/"
strip "${BUILD_DIR}/${RUNTIME_DIR}/LyMonS" 2>/dev/null || echo "Strip failed (non-fatal, cross-compiled binary)"

# Copy runtime assets and resources
echo "Copying runtime assets and resources..."
cp -fr assets/ "${BUILD_DIR}/${RUNTIME_DIR}/"
cp -fr data/ "${BUILD_DIR}/${RUNTIME_DIR}/"
cp -fr fonts/ "${BUILD_DIR}/${RUNTIME_DIR}/"

# Copy plugins
echo "Copying plugins..."
if ls target/${TARGET}/release/drivers/liblymons_driver_*.so 1>/dev/null 2>&1; then
    cp target/${TARGET}/release/drivers/liblymons_driver_*.so "${BUILD_DIR}/${RUNTIME_DIR}/drivers/"
    strip "${BUILD_DIR}/${RUNTIME_DIR}/drivers/"*.so 2>/dev/null || echo "Strip plugins failed (non-fatal, cross-compiled binary)"
else
    echo "Warning: No plugin drivers found in target/${TARGET}/release/drivers/"
fi

# Copy configuration template
cat > "${BUILD_DIR}/${RUNTIME_DIR}/config/lymons.yaml.example" <<'EOF'
# LyMonS Configuration Example for piCorePlayer

display:
  folder: /mnt/mmcblk0p2/tce/lymons/drivers/
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

# Create runtime convenience script (lives inside the lymons folder)
cat > "${BUILD_DIR}/${RUNTIME_DIR}/gomonitor" <<'EOF'
#!/bin/sh
# LyMonS for piCorePlayer - it's worth the squeeze
echo "LyMonS for piCorePlayer - it's worth the squeeze"
BINDIR="/mnt/mmcblk0p2/tce/lymons"
PNAME=`cat /usr/local/sbin/config.cfg | grep "^NAME=" | cut -d'"' -f2`

sudo killall LyMonS > /dev/null 2>&1
sudo killall LyMonS > /dev/null 2>&1
sudo killall LyMonS > /dev/null 2>&1

# wait for squeeze to come online
until pids=$(pidof squeezelite squeezelite-dsd)
do
  echo "Waiting for squeezelite ..."
  sleep 1
done
echo "Start LyMonS. Player: ${PNAME}"

VIZ=""
if [ $# -gt 0 ]; then
  VIZ="-a \"${1:-NA}\" "
fi

sudo modprobe i2c_dev > /dev/null 2>&1
sudo modprobe i2c-dev > /dev/null 2>&1
cd "${BINDIR}"
CMD="sudo ${BINDIR}/LyMonS --name \"${PNAME}\" --config=${BINDIR}/config/lymons.yaml ${VIZ} $2 $3 $4 $5 $6 $7 $8 $9"
echo $CMD
eval $CMD > /dev/null &
exit
EOF
chmod +x "${BUILD_DIR}/${RUNTIME_DIR}/gomonitor"

# Create installation script
# Package is designed to be extracted directly to /
# tar xzf lymons-*-pcp-*.tgz -C /
cat > "${BUILD_DIR}/install.sh" <<'EOF'
#!/bin/sh
# LyMonS Installation Script for piCorePlayer
# Extract the package to / first, then run this script:
#   tar xzf lymons-*-pcp-*.tgz -C /
#   sudo ./install.sh

echo "Installing LyMonS for piCorePlayer..."
RUNTIME_DIR="/mnt/mmcblk0p2/tce/lymons"

# Load i2c tools
tce-load -i i2c-tools.tcz

chmod +x "${RUNTIME_DIR}/LyMonS"
chmod +x "${RUNTIME_DIR}/gomonitor"

if [ ! -f "${RUNTIME_DIR}/config/lymons.yaml" ]; then
    cp "${RUNTIME_DIR}/config/lymons.yaml.example" "${RUNTIME_DIR}/config/lymons.yaml"
    echo "Created default configuration at ${RUNTIME_DIR}/config/lymons.yaml"
    echo "Please edit this file with your settings"
fi

# Make persistent on piCorePlayer (avoid duplicate entries)
if [ -f /usr/local/sbin/filetool.sh ]; then
    grep -v "^${RUNTIME_DIR}" /opt/.filetool.lst > /tmp/.filetool.lst.tmp \
        && mv /tmp/.filetool.lst.tmp /opt/.filetool.lst
    echo "${RUNTIME_DIR}" >> /opt/.filetool.lst
    echo "Added ${RUNTIME_DIR} to piCorePlayer backup list"
    pcp bu
    echo "Backup completed"
fi

echo ""
echo "Installation complete!"
echo ""
echo "Next steps:"
echo "1. Edit ${RUNTIME_DIR}/config/lymons.yaml with your settings"
echo "2. Test: ${RUNTIME_DIR}/gomonitor"
echo "3. Add to autostart via the piCorePlayer web interface:"
echo "   ${RUNTIME_DIR}/gomonitor &"
EOF
chmod +x "${BUILD_DIR}/install.sh"

# Create package README
cat > "${BUILD_DIR}/README.md" <<EOF
# LyMonS ${VERSION} for piCorePlayer (${ARCH})

Dynamic OLED display driver for Logitech Media Server / piCorePlayer.

**Architecture**: ${TARGET}
**Install path**: /mnt/mmcblk0p2/tce/lymons/

## Installation

1. Extract this package to the root filesystem:
   \`\`\`bash
   tar xzf ${PACKAGE_NAME}.tgz -C /
   \`\`\`

2. Run the installation script:
   \`\`\`bash
   sudo ./install.sh
   \`\`\`

3. Edit configuration:
   \`\`\`bash
   sudo nano /mnt/mmcblk0p2/tce/lymons/config/lymons.yaml
   \`\`\`

4. Test:
   \`\`\`bash
   /mnt/mmcblk0p2/tce/lymons/gomonitor
   \`\`\`

## Supported Displays

- **SSD1306** - 128x64 I2C (most common)
- **SSD1309** - 128x64 I2C
- **SH1106**  - 132x64 I2C
- **SSD1322** - 256x64 SPI (grayscale)

## Autostart on piCorePlayer

Configure through the piCorePlayer web interface as a startup command:
\`\`\`bash
/mnt/mmcblk0p2/tce/lymons/gomonitor &
\`\`\`

## Files

- \`/mnt/mmcblk0p2/tce/lymons/LyMonS\` - Main binary
- \`/mnt/mmcblk0p2/tce/lymons/gomonitor\` - Launch script
- \`/mnt/mmcblk0p2/tce/lymons/drivers/\` - Plugin drivers
- \`/mnt/mmcblk0p2/tce/lymons/data/\` - Runtime data
- \`/mnt/mmcblk0p2/tce/lymons/assets/\` - Runtime assets
- \`/mnt/mmcblk0p2/tce/lymons/fonts/\` - Fonts
- \`/mnt/mmcblk0p2/tce/lymons/config/lymons.yaml.example\` - Configuration template
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

BINARY_SIZE=$(du -h "${BUILD_DIR}/${RUNTIME_DIR}/LyMonS" | cut -f1)
PACKAGE_SIZE=$(du -h "${OLDPWD}/${PACKAGE_NAME}.tgz" | cut -f1)

echo ""
echo "✓ Package created: ${PACKAGE_NAME}.tgz"
echo "  Binary size: ${BINARY_SIZE}"
echo "  Package size: ${PACKAGE_SIZE}"
echo ""
echo "To deploy:"
echo "  tar xzf ${PACKAGE_NAME}.tgz -C /"
echo "  sudo ./install.sh"
