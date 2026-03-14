/*
 *  lib.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Library crate root — module declarations.
 */

pub mod config;
pub mod dbfs;
pub mod draw;
pub mod drawsvg;
pub mod display;
pub mod mac_addr;
pub mod metrics;
pub mod const_oled;
pub mod constants;
pub mod glyphs;
pub mod clock_font_svg;
pub mod deutils;
pub mod httprpc;
pub mod sliminfo;
pub mod weather;
pub mod textable;
pub mod weather_glyph;
pub mod geoloc;
pub mod location;
pub mod astral;
pub mod translate;
pub mod eggs;
pub mod spectrum;
pub mod vframebuf;
pub mod vision;
pub mod visualization;
pub mod visualizer;
pub mod sse_client;
pub mod visionon;
pub mod vuphysics_new;
pub mod svgimage;
pub mod shm_path;
pub mod sun;
pub mod coverart;

include!(concat!(env!("OUT_DIR"), "/build_info.rs"));
