# SHARP Memory LCD Assets (400x240)

This directory contains assets optimized for large SHARP Memory LCD displays and similar high-resolution displays (400x240 or larger).

## Required Assets

### Visualizer Panels
These SVG files should be scaled for 400x240 resolution:

- `vu2up.svg` - Dual VU meters (stereo)
- `vudownmix.svg` - Single VU meter (mono)
- `vucombi.svg` - VU meters with center peak meter
- `vuaio.svg` - All-in-one VU meter display
- `peak.svg` - Peak meters (stereo)
- `peakmono.svg` - Peak meter (mono)
- `histaio.svg` - All-in-one histogram display

### Design Guidelines

**Target Resolution**: 400x240 pixels
**Aspect Ratio**: 5:3 (wider than typical OLEDs)
**Bit Depth**: Monochrome (1-bit) for SHARP Memory LCDs
**Content Area**: ~390x220 pixels (accounting for status bar)

### Scaling Recommendations

When creating these assets from the 128x64 or 256x64 versions:

1. **Scale Factor**: Approximately 3.1x from 128x64, or 1.56x from 256x64
2. **Line Weights**: Increase proportionally to maintain visibility
3. **Text**: Use larger fonts or vector text that scales cleanly
4. **Detail Level**: Can include more detail due to larger canvas
5. **VU Segments**: Increase to ~40 segments for smoother appearance
6. **Peak Meters**: Increase height to ~20 pixels for better visibility

### File Format

- **Format**: SVG (Scalable Vector Graphics)
- **Optimization**: Minimize file size while maintaining quality
- **Compatibility**: Ensure compatibility with `resvg` crate for rendering

## Status

**Currently**: Placeholder directory for future implementation
**TODO**: Create scaled versions of all visualizer panels for 400x240 resolution

## Testing

Once assets are created, test with:
```bash
cargo build --release
# Configure display.driver = "sharpmemory" in config
# Configure display.width = 400, display.height = 240
```

The layout system will automatically select assets from this directory when a 400x240 or larger display is detected.
