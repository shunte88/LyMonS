# LyMonS
An LMS Monitor For The Future
OLED information display control program for [piCorePlayer](https://www.picoreplayer.org/) or other Raspberry Pi and Lyrion Music Server (formerly Logitech Media Server) based audio device.

<img width="800" src="assets/lymons.webp" align="center" />

### Features
- Track details are displayed only when playing
- Display features independant scrolling of track details when required.
- Remaining time can now be displayed rather than total time
- Audio attributes, volume, sample depth, and sample rate are shown
- A retro clock is displayed when the audio paused/stopped.
- You can display current weather and time.
- Automatically sets the brightness of the display at dawn and dusk.
- Multiple audio visualization modes are supported
- Multiple visualization styles are supported
- If monitoring from a separate device animations can be displayed as the track plays
- Alternatively can also be displayed instead of a visualization as the track plays

### Options
```bash
Usage: lymons --name "NAME" [OPTIONS...]
OLED information display for piCorePlayer or other Raspberry Pi and LMS based audio device.

  -n, --name PLAYERNAME      Name of the squeeze device to monitor
  -o, --oled [OLEDTYPE]      Specify OLED "driver" type (see options below)
  -r, --remain-time          Display remaining time rather than track time
  -S, --scroll [SCROLLMODE]  Label scroll mode: cylon, or loop
  -V, --verbose              Maximum log level
  -z, --splash               Show Splash Screen

Supported OLED types:
    SH1106
    SSD1306
    SSD1309
    SSD1322

OLED Clock Fonts:
    7seg ........: Classic LCD Clock Font
    soldeco .....: Deco-Solid Font
    holdeco .....: Deco-Hollow Font
    holfestus ...: Festus Hollow 25x44
    solfestus ...: Festus Solid 25x44
    space1999 ...: Space 1999
    roboto ......: Roboto Thin
    solnoto .....: noto 25x44
    holnoto .....: noto fancy 25x44

```

### Visualizer Modes

Several visualizer modes are supported
- Stereo VU Meters - dBfs metered
- Stereo 12-band Spectrum Analysis
- Stereo 12-band "tornado" Spectrum Analysis
- Stereo 12-band "mirror" Spectrum Analysis
- Stereo Peak Meter - dBfs metered
- Downmix (visual data only) Peak Meter
- Large Downmix (visual data only) VU meter
- Large Downmix (visual data only) Spectrum
- All-In-One - track details and spectrum/VU "swoosh" (use -a1 or simply -a)
- All-In-One - fixed mode (use -a2 or simply -a -a)
- Easter Eggs - fixed mode (use -E[1-7])

### Installation

There are two modes of operation:

- LyMonS installed on piCore Player, consuming visualization data directly
- LyMonS installed on an alternate device, the LMS Server for example, consuming streamed visualization data

# Prerequisites

If you are intending to consume visualization data you need to configure squeezelite to expose the shared memory

From the Squeezlite page of the pCP web frontend type 1 in the "m" ALSA parameter section

And, in the Various Options add *-v*

See the squeezelite page for more details

## Easter Eggs
<p>
<img width="130" align="right" src="assets/eastereggs.jpg"/>

There are several "easter egg" modes provided for those setups that cannot process the audio data for visualization.
That said theres nothing stopping you using them as your main visualization.

There are currently 7 easter egg modes:
- <b>[1]</b> Compact Cassette, as visually correct as possible given the OLED limitations.  Hubs turn and the tape window shows the track "progress"
- <b>[2]</b> Technics SL-1200, as visually correct as possible given the OLED limitations.  Tone arm traverses platter to indicate progress.
- <b>[3]</b> Open Reel To Reel, pure fantasy. Reels rotate, minor animation.
- <b>[4]</b> VCR with flashing 12:00 AM clock! No additional animation - the clock is annoying enough.
- <b>[5]</b> An old bakelite radio. Minor animation, radio changes station as track progresses.
- <b>[6]</b> An old analog TV in all its 5x4 glory... VHF or UHF... no it's worms?!?
- <b>[7]</b> A crusty old IBM PS/2 clone... playing pong! Equally matched "AI" players make for an uneventful game until they cheat!

Specify -E[1-7] to display eggs on track playback
</p>
These are just a fun display mode where visualization is not possible.

### Coming soon

- TODO! Audio visualizer support: stereo VU meters
- TODO! Audio visualizer support: histogram spectrum
- TODO! Audio visualizer support: horizontal Peak RMS
- TODO! Set display brightness, day and night modes.
- TODO! Downmix visual data and display on one large VU meter.
- TODO! Downmix visual data and display on one large histogram.
- TODO! Weather: free API incorporation
- TODO! Weather: current and forecast support
- TODO! Downmix PK Meter - scratch draw and animation
- TODO! SSD1322 256x64 OLED support - WIP

## Like The App - Git The Shirt

Team Badger shirts and other goodies are available at [shunte88](https://www.zazzle.com/team_badger_t_shirt-235604841593837420)

