# LyMonS Emulation Mode Testing

## Overview

This document describes how to test LyMonS functionality in emulation mode on Ubuntu using a local Slimserver (Logitech Media Server) and Squeezelite.

## Prerequisites

### Install Logitech Media Server (Slimserver)

```bash
# Download latest LMS from https://downloads.slimdevices.com/
wget http://downloads.slimdevices.com/LogitechMediaServer_v8.4.0/logitechmediaserver_8.4.0_amd64.deb
sudo dpkg -i logitechmediaserver_8.4.0_amd64.deb
sudo apt-get install -f  # Fix dependencies

# Start LMS
sudo systemctl start logitechmediaserver
sudo systemctl enable logitechmediaserver

# Access web interface
firefox http://localhost:9000
```

### Install Squeezelite

```bash
# Install squeezelite player
sudo apt-get install squeezelite

# Start squeezelite
squeezelite -n "TestPlayer" -o pulse
```

Or build from source for latest version:

```bash
git clone https://github.com/ralph-irving/squeezelite.git
cd squeezelite
make
./squeezelite -n "TestPlayer" -o pulse
```

## LyMonS Emulator Mode

### Build with Emulator

```bash
cd /data2/refactor/LyMonS
cargo build --release --features emulator
```

### Configuration for Emulation

Create `lymons-emulator.yaml`:

```yaml
display:
  driver: ssd1306          # Or any supported driver
  emulated: true           # Enable emulator window
  bus:
    type: i2c
    bus: "/dev/i2c-1"      # Not used in emulation
    address: 0x3C
  brightness: 128
  rotate_deg: 0

slimserver:
  host: "localhost"
  port: 9000

# Optional: specify player MAC
# player_mac: "00:11:22:33:44:55"
```

### Run Emulator

```bash
# Run the LyMonS binary with emulator feature
./target/release/LyMonS --config lymons-emulator.yaml

# Or run directly with cargo
cargo run --release --features emulator -- --config lymons-emulator.yaml
```

**Note:** There is only ONE binary (`LyMonS`). Emulation mode is enabled by:
1. Building with `--features emulator`
2. Setting `emulated: true` in the configuration file

## Testing Workflow

### 1. Start Services

```bash
# Terminal 1: Start LMS (if not already running)
sudo systemctl start logitechmediaserver

# Terminal 2: Start Squeezelite
squeezelite -n "TestPlayer" -o pulse -d all=info

# Terminal 3: Start LyMonS
cargo run --release --features emulator -- --config lymons-emulator.yaml
```

### 2. Connect to LMS Web Interface

```bash
firefox http://localhost:9000
```

1. Click on "Settings" → "Players"
2. You should see "TestPlayer" listed
3. Click on the player to select it

### 3. Play Music

1. Add some music to LMS library:
   - Settings → Basic Settings → Media Folder
   - Point to your music directory
   - Rescan library

2. Browse and play music through web interface
3. Watch LyMonS emulator window update with:
   - Track title
   - Artist name
   - Album art (if available)
   - Playback status
   - Volume meters

### 4. Test Display Features

**Now Playing Display:**
- Play a track
- Verify title scrolls if too long
- Verify artist name displays
- Check album art rendering

**Visualizer Mode:**
- While playing, LyMonS should show audio visualizer
- VU meters should respond to audio
- Spectrum analyzer should show frequency distribution

**Clock Mode:**
- When not playing, should display clock
- Time should update every second

**Weather Mode** (if configured):
- Add weather API key to config
- Verify weather data displays

### 5. Test Plugin System

```bash
# Build all plugins
make plugins

# Verify plugins are loaded
ls -lh target/release/drivers/

# Run with plugin system
cargo run --release --features plugin-system,emulator -- --config lymons-emulator.yaml

# Check logs for plugin loading
# Should see: "Loaded plugin: LyMonS SSD1306 Driver v1.0.0"
```

### 6. Test Different Drivers

**SSD1306 (128x64 monochrome):**
```yaml
display:
  driver: ssd1306
  emulated: true
```

**SSD1309 (128x64 monochrome):**
```yaml
display:
  driver: ssd1309
  emulated: true
```

**SH1106 (132x64 monochrome):**
```yaml
display:
  driver: sh1106
  emulated: true
```

**SSD1322 (256x64 grayscale):**
```yaml
display:
  driver: ssd1322
  emulated: true
```

## Emulator Window Features

### Display

- Window size matches configured display resolution
- Pixel-perfect rendering
- Monochrome or grayscale based on driver

### Controls

- **ESC** - Exit emulator
- **F** - Toggle fullscreen
- **R** - Rotate display 90°
- **B** - Cycle brightness
- **I** - Toggle invert

### Debug Info

- FPS counter
- Plugin name and version
- Driver capabilities
- Current mode

## Troubleshooting

### Squeezelite Not Appearing in LMS

```bash
# Check squeezelite is running
ps aux | grep squeezelite

# Check network interface
squeezelite -n "TestPlayer" -o pulse -d all=info

# Try explicit server
squeezelite -n "TestPlayer" -o pulse -s localhost
```

### LyMonS Can't Connect to LMS

```bash
# Verify LMS is running
systemctl status logitechmediaserver

# Check LMS web interface
curl http://localhost:9000

# Test JSON-RPC
curl -X POST http://localhost:9000/jsonrpc.js \
  -d '{"method":"slim.request","params":["",["serverstatus",0,999]]}'
```

### No Audio in Squeezelite

```bash
# List audio devices
squeezelite -l

# Try specific device
squeezelite -n "TestPlayer" -o default

# Or use ALSA
squeezelite -n "TestPlayer" -o hw:CARD=PCH,DEV=0
```

### Plugin Not Loading

```bash
# Check plugin exists
ls -l target/release/drivers/

# Check plugin symbols
nm -D target/release/drivers/liblymons_driver_ssd1306.so | grep lymons_plugin_register

# Run with debug logging
RUST_LOG=debug cargo run --release --features plugin-system,emulator
```

## Performance Testing

### Frame Rate

```bash
# Should maintain 60 FPS for monochrome displays
# Should maintain 30+ FPS for grayscale displays
# Monitor FPS counter in emulator window
```

### Memory Usage

```bash
# Check memory usage
ps aux | grep LyMonS

# Should be < 50 MB typically
```

### CPU Usage

```bash
# Monitor CPU
top -p $(pgrep LyMonS)

# Should be < 10% on modern CPU
```

## Automated Testing

### Test Script

```bash
#!/bin/bash
# test-emulator.sh

# Start services
systemctl start logitechmediaserver
squeezelite -n "TestPlayer" -o pulse &
SQUEEZE_PID=$!

# Wait for services
sleep 5

# Run LyMonS with timeout
timeout 60 cargo run --release --features emulator &
LYMONS_PID=$!

# Wait
sleep 60

# Cleanup
kill $LYMONS_PID $SQUEEZE_PID
```

## Integration with Real Hardware

Once emulation testing is successful, deploy to real hardware:

```bash
# Build for ARM (Pi)
cargo build --release --target armv7-unknown-linux-musleabihf

# Copy to Pi
scp target/armv7-unknown-linux-musleabihf/release/LyMonS pi@raspberrypi:~/

# Copy plugins
scp target/release/drivers/*.so pi@raspberrypi:~/.local/lib/lymons/drivers/

# SSH and test
ssh pi@raspberrypi
./LyMonS --config lymons.yaml
```

## Success Criteria

- ✅ LMS starts and web interface accessible
- ✅ Squeezelite connects to LMS
- ✅ LyMonS connects to LMS
- ✅ LyMonS detects player
- ✅ Now playing information displays
- ✅ Album art renders correctly
- ✅ Visualizer responds to audio
- ✅ Clock mode shows when stopped
- ✅ Plugin system loads drivers
- ✅ All 4 display drivers work
- ✅ Emulator window displays correctly
- ✅ 60 FPS maintained
- ✅ < 10% CPU usage
- ✅ < 50 MB memory usage

## Next Steps

After successful emulation testing:

1. Test on real hardware (Raspberry Pi)
2. Test with real OLED displays
3. Create systemd service
4. Package for PiCorePlayer
5. Test on TinyCore Linux

---

**Testing Date:** 2026-02-01
**LyMonS Version:** 0.2.1
**Status:** Ready for Testing
