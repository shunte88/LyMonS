# LyMonS SSD1306 Display Driver Plugin

A dynamic plugin for LyMonS that provides SSD1306 OLED display driver support.

## Features

- **Display:** 128x64 monochrome OLED
- **Interface:** I2C
- **Brightness:** 4 levels (DIMMEST, DIM, NORMAL, BRIGHTEST)
- **Rotation:** 0°, 90°, 180°, 270°
- **Inversion:** Yes
- **Max FPS:** 60

## Hardware Requirements

- SSD1306-based 128x64 OLED display
- I2C interface
- Common I2C addresses: 0x3C, 0x3D

## Building

Build the plugin as a shared library:

```bash
cargo build --release
```

Output: `target/release/liblymons_ssd1306.so`

## Installation

### Development

For development, LyMonS automatically discovers plugins in `./target/release/drivers/`:

```bash
mkdir -p target/release/drivers
cp target/release/liblymons_ssd1306.so target/release/drivers/
```

### System-Wide

```bash
sudo mkdir -p /usr/local/lib/lymons/drivers
sudo cp target/release/liblymons_ssd1306.so /usr/local/lib/lymons/drivers/
```

### User-Local

```bash
mkdir -p ~/.local/lib/lymons/drivers
cp target/release/liblymons_ssd1306.so ~/.local/lib/lymons/drivers/
```

## Configuration

Configure LyMonS to use the SSD1306 driver:

```yaml
display:
  driver: ssd1306
  bus:
    type: i2c
    bus: "/dev/i2c-1"
    address: 0x3C
  brightness: 128      # Optional: 0-255
  rotate_deg: 0        # Optional: 0, 90, 180, 270
  invert: false        # Optional: true/false
```

## Plugin Interface

This plugin implements the LyMonS Plugin ABI v1.0.0:

- **Entry Point:** `lymons_plugin_register()`
- **ABI Version:** 1.0.0
- **Driver Type:** ssd1306

## Dependencies

- `ssd1306` (0.10.0) - SSD1306 driver
- `embedded-hal` (1.0.0) - Hardware abstraction
- `linux-embedded-hal` (0.4.0) - Linux I2C support
- `embedded-graphics` (0.8.1) - Graphics primitives

## License

GPL-3.0-or-later

## Author

Stuart Hunter

## Version

1.0.0
