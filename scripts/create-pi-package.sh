#!/bin/bash
# Create Raspberry Pi deployment package (.tgz)
# For standard Raspberry Pi Linux (non-pCP) deployment

set -e

# Get target from argument or default to armv7
TARGET="${1:-armv7-unknown-linux-gnueabihf}"
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
ARCH=$(echo "${TARGET}" | cut -d'-' -f1)
# The ARMv6 target triple is "arm-*"; present it to users as "armv6"
[ "${ARCH}" = "arm" ] && ARCH="armv6"
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
    echo "Plugins included:"
    for so in "${BUILD_DIR}/${RUNTIME_DIR}/drivers/"*.so; do
        echo "  $(basename "${so}")"
    done
else
    echo "Warning: No plugin drivers found in target/${TARGET}/release/drivers/"
fi
# Plugins are a fallback for binaries built without a given driver.  Every
# driver is compiled into the standard binary, so a driver with no plugin here
# is not a missing feature.  See "Supported Displays" in the README.
echo "All display drivers are built into the binary; plugins are optional."

# Copy the bus discovery helper (shared with the pCP and Orange Pi packages)
if [ ! -f scripts/show-buses.sh ]; then
    echo "Error: scripts/show-buses.sh not found (run this from the repo root)"
    exit 1
fi
cp scripts/show-buses.sh "${BUILD_DIR}/${RUNTIME_DIR}/show-buses.sh"
chmod +x "${BUILD_DIR}/${RUNTIME_DIR}/show-buses.sh"

# Copy configuration template (config lives in the application folder)
cat > "${BUILD_DIR}/${RUNTIME_DIR}/config/lymons.yaml.example" <<'EOF'
# LyMonS Configuration Example for Raspberry Pi

display:
  folder: /usr/local/share/lymons/drivers/

  # Options: ssd1306, ssd1309, sh1106, sh1107, ssd1322, sh1122,
  #          st7789, st7796s, sharpmemory
  driver: ssd1306

  # Only needed for drivers offering more than one panel size (sh1107, st7789).
  # Single-size drivers ignore these. See the Supported Displays table in
  # README.md for the sizes each driver accepts.
  # width: 128
  # height: 64

  # --- I2C: ssd1306, ssd1309, sh1106, sh1107 ---------------------------------
  bus:
    type: i2c
    bus: "/dev/i2c-1"      # Pi header I2C is bus 1
    address: 0x3C          # Common: 0x3C or 0x3D

  # --- SPI: any driver above, plus ssd1322, sh1122, st7789, st7796s and
  #          sharpmemory, which are SPI only ------------------------------
  # Replace the i2c block above with this for a 4-wire SPI panel. On a Pi the
  # whole 40-pin header is a single GPIO controller whose line offsets are the
  # BCM numbers, so dc_pin/rst_pin below are BCM 24 and BCM 25 and gpio_chip
  # can be left unset (the header controller is detected automatically).
  #
  # bus:
  #   type: spi
  #   bus: "/dev/spidev0.0"
  #   dc_pin: 24
  #   rst_pin: 25
  #   speed_hz: 8000000

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
# LyMonS for Raspberry Pi
# This LyMonS worth the squeeze
BINDIR="/usr/local/share/lymons"
PNAME=""
if [ -f "/usr/local/sbin/config.cfg" ]; then
  PNAME=`cat /usr/local/sbin/config.cfg | grep "^NAME=" | cut -d'"' -f2`
fi

sudo killall LyMonS > /dev/null 2>&1
sudo killall LyMonS > /dev/null 2>&1
sudo killall LyMonS > /dev/null 2>&1

if [ -n "${PNAME}" ]; then
  # wait for squeeze to come online
  until pids=$(pidof squeezelite squeezelite-dsd)
  do
    echo "Waiting for squeezelite ..."
    sleep 1
  done
fi
echo "Start LyMonS. Player: ${PNAME}"

if [ -n "${PNAME}" ]; then
  sudo modprobe i2c_dev > /dev/null 2>&1
  sudo modprobe i2c-dev > /dev/null 2>&1
fi

cd "${BINDIR}" || {
    echo "Error: runtime folder ${BINDIR} not found."
    echo "LyMonS resolves assets, data and fonts relative to it."
    exit 1
}
CMD="sudo /usr/local/bin/LyMonS --config=${BINDIR}/config/lymons.yaml $@"
echo $CMD
eval $CMD > /dev/null &
exit
EOF
chmod +xX "${BUILD_DIR}/${RUNTIME_DIR}/gomonitor"

# Create installation script
cat > "${BUILD_DIR}/install.sh" <<'EOF'
#!/bin/sh
# LyMonS Installation Script for Raspberry Pi
# This LyMonS worth the squeeze
# Run as: sudo ./install.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUNTIME_DIR="/usr/local/share/lymons"
BIN_DIR="/usr/local/bin"
PAYLOAD="${SCRIPT_DIR}/usr/local/share/lymons"

echo "Installing LyMonS for Raspberry Pi..."
echo ""
echo "  Runtime : ${RUNTIME_DIR}"
echo "  Binary  : ${BIN_DIR}/LyMonS"
echo "  Config  : ${RUNTIME_DIR}/config/lymons.yaml"
echo ""

# The payload travels with this script.  Copying from anywhere else silently
# installs nothing, which shows up much later as a missing asset at runtime.
if [ ! -d "${PAYLOAD}" ]; then
    echo "Error: package payload not found at ${PAYLOAD}"
    echo "Run install.sh from the directory the tarball extracted into."
    exit 1
fi

# Deploy binary
mkdir -p "${BIN_DIR}"
cp -f "${SCRIPT_DIR}/usr/local/bin/LyMonS" "${BIN_DIR}/LyMonS"
chmod +x "${BIN_DIR}/LyMonS"
echo "Installed binary: ${BIN_DIR}/LyMonS"

# Deploy runtime folder (data, assets, fonts, drivers, gomonitor, config)
mkdir -p "${RUNTIME_DIR}"
cp -fr "${PAYLOAD}/." "${RUNTIME_DIR}/"
chmod +x "${RUNTIME_DIR}/gomonitor"
chmod +x "${RUNTIME_DIR}/show-buses.sh"
echo "Installed runtime resources to ${RUNTIME_DIR}/"

# Create config from example if not already present
if [ ! -f "${RUNTIME_DIR}/config/lymons.yaml" ]; then
    cp "${RUNTIME_DIR}/config/lymons.yaml.example" "${RUNTIME_DIR}/config/lymons.yaml"
    echo "Created default configuration at ${RUNTIME_DIR}/config/lymons.yaml"
    echo "Please edit this file with your settings"
fi

# Confirm the runtime tree landed.  LyMonS resolves assets, data and fonts
# relative to its working directory, so a partial copy is not visible until
# the display is already up and a page asks for a file that is not there.
MISSING=""
for d in assets data fonts drivers config; do
    [ -d "${RUNTIME_DIR}/${d}" ] || MISSING="${MISSING} ${d}/"
done
for f in assets/layout.yaml assets/none.svg data/7seg.zip; do
    [ -f "${RUNTIME_DIR}/${f}" ] || MISSING="${MISSING} ${f}"
done
if [ -n "${MISSING}" ]; then
    echo ""
    echo "Error: installation incomplete, missing:${MISSING}"
    exit 1
fi

# piCorePlayer persistence (if applicable)
if [ -f /usr/local/sbin/filetool.sh ] && [ -f /opt/.filetool.lst ]; then
    grep -v "^${RUNTIME_DIR}" /opt/.filetool.lst > /tmp/.filetool.lst.tmp || true
    mv /tmp/.filetool.lst.tmp /opt/.filetool.lst
    echo "${BIN_DIR}/LyMonS" >> /opt/.filetool.lst
    echo "${RUNTIME_DIR}" >> /opt/.filetool.lst
    echo "Added to piCorePlayer backup list"
    filetool.sh -b
fi

echo ""
echo "Installation complete!"
echo ""
echo "Next steps:"
echo "1. See what buses this board exposes: ${RUNTIME_DIR}/show-buses.sh"
echo "2. Edit ${RUNTIME_DIR}/config/lymons.yaml with your settings"
echo "3. Test: ${RUNTIME_DIR}/gomonitor"
echo "4. Add to autostart if desired"
EOF
chmod +x "${BUILD_DIR}/install.sh"

# Create package README
cat > "${BUILD_DIR}/README.md" <<EOF
# LyMonS ${VERSION} for Raspberry Pi (${ARCH})

Dynamic OLED display driver for Lyrion Media Server.

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

If the display stays dark, run the bus discovery helper first, it prints the
I2C buses, SPI nodes and GPIO controllers this board actually has:

\`\`\`bash
/usr/local/share/lymons/show-buses.sh
\`\`\`

## Supported Displays

Color depth is a property of the driver, not the panel size.

| Driver        | Bus     | Color depth    | Panel sizes                                         |
|---------------|---------|----------------|-----------------------------------------------------|
| \`ssd1306\`     | I2C/SPI | Mono (1bpp)    | 128x64                                              |
| \`ssd1309\`     | I2C/SPI | Mono (1bpp)    | 128x64                                              |
| \`sh1106\`      | I2C/SPI | Mono (1bpp)    | 132x64                                              |
| \`sh1107\`      | I2C/SPI | Mono (1bpp)    | 128x128, 128x64                                     |
| \`ssd1322\`     | SPI     | Gray4 (4bpp)   | 256x64                                              |
| \`sh1122\`      | SPI     | Gray4 (4bpp)   | 256x64                                              |
| \`st7789\`      | SPI     | Rgb565 (16bpp) | 320x240, 320x170, 280x240, 240x240, 240x135, 160x80 |
| \`st7796s\`     | SPI     | Rgb565 (16bpp) | 480x320                                             |
| \`sharpmemory\` | SPI     | Mono (1bpp)    | 400x240                                             |

Set \`width\` and \`height\` in the \`display:\` block for the drivers that offer more
than one panel size. The rest fix their own size and ignore the values.

## Bus Interfaces

Enable the bus you need with \`sudo raspi-config\` (Interface Options), then reboot.
Run \`show-buses.sh\` afterwards to confirm the device nodes appeared.

- **I2C**: \`/dev/i2c-1\` on the 40-pin header. Address is usually 0x3C, sometimes 0x3D.
- **SPI**: \`/dev/spidev0.0\` or \`/dev/spidev0.1\`, plus two GPIO lines for DC and
  reset. Pi header pins are BCM numbers, so \`dc_pin: 24\` is BCM 24; the header
  GPIO controller is detected automatically and \`gpio_chip\` is not needed.
  On non-Pi boards it is, see the Orange Pi package or README.md.

## Files

- \`/usr/local/bin/LyMonS\` - Main binary
- \`/usr/local/share/lymons/gomonitor\` - Launch script
- \`/usr/local/share/lymons/show-buses.sh\` - Bus and GPIO discovery helper
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
