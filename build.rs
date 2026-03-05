// build.rs

use chrono::Utc;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Get the output directory set by Cargo
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_info.rs");

    // Get version from Cargo.toml (set automatically by Cargo)
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());

    // Get the current UTC time
    let now = Utc::now();
    let build_date = now.format("%Y-%m-%d %H:%M:%S UTC").to_string();
    let build_date_short = now.format("%Y-%m-%d").to_string();

    // Write build info constants
    fs::write(
        &dest_path,
        format!(
            "pub const BUILD_DATE: &str = \"{}\";\npub const VERSION: &str = \"{}\";\n",
            build_date, version
        ),
    )
    .unwrap();

    // Generate the version badge SVG in the project root
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let svg_path = Path::new(&manifest_dir).join("version.svg");

    let value = format!("LyMonS Version: {} | Built: {}", version, build_date_short);

    // Measure text widths (approximate: 6.4px per char at 11px font)
    let char_width = 6.4_f64;
    let padding = 12.0_f64;
    let value_width = (value.len() as f64 * char_width + padding * 2.0).round();

    let svg = format!(
        r##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<svg
   width="{value_w}" height="20" role="img" aria-label="{value}"
   version="1.1" id="svg01"
   xmlns="http://www.w3.org/2000/svg"
   xmlns:svg="http://www.w3.org/2000/svg">
  <title
     id="title1">{value}</title>
  <defs
     id="defs1">
    <linearGradient
       id="grad01"
       x2="0"
       y2="78.993668"
       gradientTransform="matrix(3.9496835,0,0,0.25318484,2.2538372,-53.528634)"
       x1="0"
       y1="0"
       gradientUnits="userSpaceOnUse">
      <stop
         offset="0"
         stop-color="#bbbbbb"
         stop-opacity=".1"
         id="stop1"
         style="stop-color:#000000;stop-opacity:0.1;" />
      <stop
         offset="1"
         stop-opacity=".1"
         id="stop2" />
    </linearGradient>
    <clipPath
       id="clipper">
      <rect
         width="{value_w}"
         height="20"
         rx="3"
         fill="#ffffff"
         id="rect2"
         x="0"
         y="0" />
    </clipPath>
  </defs>
  <g
     clip-path="url(#clipper)"
     id="g5">
    <rect
       width="{value_w}"
       height="20"
       fill="#555555"
       id="rect3"
       x="0"
       y="0" />
    <rect
       x="{value_w}"
       width="{value_w}"
       height="20"
       fill="#44cc11"
       id="rect4"
       y="0" />
    <rect
       width="{value_w}"
       height="20"
       fill="url(#grad01)"
       id="rect5"
       style="fill:url(#grad01);stroke:#000000;stroke-opacity:1"
       x="2.2538373"
       y="-53.528633" />
 <text
     xml:space="preserve"
     style="font-style:normal;font-size:10px;font-family:'DejaVu Serif';text-align:center;text-anchor:middle;fill:#FFD700;stroke:#BE8400;stroke-width:0.5;stroke-linecap:round;stroke-linejoin:round;stroke-opacity:1"
     x="{value_cx}"
     y="13"
     id="text7">&gt;&gt; {value} &lt;&lt;</text>
  </g>
</svg>"##,
        value_w = value_width,
        value_cx = (value_width * 0.5),
        value = value,
    );

    fs::write(&svg_path, svg).unwrap();

    // Tell Cargo to re-run this build script only if build.rs itself changes
    println!("cargo:rerun-if-changed=build.rs");

}


