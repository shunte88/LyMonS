/*
 *  coverart.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Cover art fetch, cache, and Rgb565 conversion.
 *
 *  Only active on large Rgb565 displays (ST7789 320×170).
 *  Uses the LMS `coverid` field as the cache key — one file per track,
 *  stored as a JPEG in `~/.cache/lymons/coverart/`.
 *
 *  Pipeline (cache miss):
 *    fetch JPEG from LMS  →  decode  →  resize 160×160 Lanczos  →
 *    save JPEG to cache   →  convert to Rgb565  →  return CoverArt
 *
 *  Pipeline (cache hit):
 *    read JPEG from cache  →  decode  →  convert to Rgb565  →  return CoverArt
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  See <http://www.gnu.org/licenses/> to get a copy of the GNU General
 *  Public License.
 *
 */

#![allow(dead_code)]

use std::io::BufWriter;
use std::path::{Path, PathBuf};

use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat};
use log::{debug, info, warn};
use thiserror::Error;

/// Target cover art dimensions (pixels, square).
pub const COVER_SIZE: u32 = 160;

/// JPEG quality for cached files (0–100).
const CACHE_JPEG_QUALITY: u8 = 75;

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CoverArtError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("No cover art available from LMS")]
    NotAvailable,
}

// ── CoverArt ─────────────────────────────────────────────────────────────────

/// COVER_SIZE × COVER_SIZE pixels in big-endian Rgb565 format.
///
/// Ready to blit to an ST7789 display via `draw_to()` or consumed as raw bytes
/// by `write_buffer()`.
pub struct CoverArt {
    /// Big-endian Rgb565 bytes: `COVER_SIZE * COVER_SIZE * 2` bytes total.
    pixels: Vec<u8>,
}

impl CoverArt {
    pub fn width() -> u32 { COVER_SIZE }
    pub fn height() -> u32 { COVER_SIZE }

    /// Raw big-endian Rgb565 bytes (2 bytes per pixel, row-major).
    pub fn as_bytes(&self) -> &[u8] { &self.pixels }

    /// Draw the cover art to any `Rgb565` `DrawTarget` at `position`.
    pub fn draw_to<D>(&self, display: &mut D, position: Point) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Rgb565>,
    {
        let raw = ImageRaw::<Rgb565>::new(&self.pixels, COVER_SIZE);
        Image::new(&raw, position).draw(display)
    }

    fn from_image(img: DynamicImage) -> Self {
        let rgb = img.to_rgb8();
        let mut pixels = Vec::with_capacity((COVER_SIZE * COVER_SIZE * 2) as usize);
        for p in rgb.pixels() {
            // RGB8 → Rgb565 big-endian
            let r = p[0] >> 3;   // 8-bit → 5-bit
            let g = p[1] >> 2;   // 8-bit → 6-bit
            let b = p[2] >> 3;   // 8-bit → 5-bit
            let word: u16 = ((r as u16) << 11) | ((g as u16) << 5) | (b as u16);
            pixels.push((word >> 8) as u8);
            pixels.push(word as u8);
        }
        Self { pixels }
    }
}

// ── CoverArtCache ─────────────────────────────────────────────────────────────

/// Disk-backed cover art cache keyed by LMS `coverid`.
///
/// # Usage
///
/// ```ignore
/// let cache = CoverArtCache::new("~/.cache/lymons/coverart")?;
/// let art = cache.get("f3a9c12b", "192.168.1.25", 9000, "b8:27:eb:70:71:5c").await?;
/// art.draw_to(&mut display, Point::new(80, 5))?;
/// ```
pub struct CoverArtCache {
    cache_dir: PathBuf,
    client: reqwest::Client,
}

impl CoverArtCache {
    /// Create (or open) a cache rooted at `cache_dir`.
    ///
    /// The directory is created if it does not already exist.
    pub fn new(cache_dir: impl AsRef<Path>) -> Result<Self, CoverArtError> {
        let cache_dir = cache_dir.as_ref().to_owned();
        std::fs::create_dir_all(&cache_dir)?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("reqwest client");
        Ok(Self { cache_dir, client })
    }

    /// Return cover art for `coverid`.
    ///
    /// On a cache miss the image is fetched from LMS, resized, saved to disk,
    /// and returned. Subsequent calls for the same `coverid` are served from
    /// the on-disk JPEG without any network access.
    ///
    /// # Arguments
    /// * `coverid`    — LMS `PlaylistLoop[0].Coverid` (used as cache filename)
    /// * `lms_host`   — LMS server IP/hostname (e.g. `"192.168.1.25"`)
    /// * `lms_port`   — LMS HTTP port (usually `9000`)
    /// * `player_mac` — Player MAC address (e.g. `"b8:27:eb:70:71:5c"`)
    pub async fn get(
        &self,
        coverid: &str,
        lms_host: &str,
        lms_port: u16,
        player_mac: &str,
    ) -> Result<CoverArt, CoverArtError> {
        // ── 1. Cache hit ──────────────────────────────────────────────────────
        let path = self.cache_path(coverid);
        if path.exists() {
            debug!("Cover art cache hit: {}", coverid);
            return self.load_from_cache(&path);
        }

        // ── 2. Cache miss — fetch from LMS ────────────────────────────────────
        info!("Cover art cache miss — fetching coverid={}", coverid);

        let primary = format!(
            "http://{}:{}/music/current/cover.jpg?player={}",
            lms_host, lms_port, player_mac
        );
        let jpeg = match self.fetch_jpeg(&primary).await {
            Ok(b) => b,
            Err(e) => {
                warn!("Primary cover art URL failed ({}), trying fallback", e);
                let fallback = format!(
                    "http://{}:{}/music/{}/cover_{}x{}_o",
                    lms_host, lms_port, coverid, COVER_SIZE, COVER_SIZE
                );
                self.fetch_jpeg(&fallback).await?
            }
        };

        self.decode_resize_cache(coverid, &jpeg, &path)
    }

    /// Purge the cached file for `coverid` (e.g. when the track changes and
    /// the art is no longer needed). No-op if not cached.
    pub fn evict(&self, coverid: &str) {
        let path = self.cache_path(coverid);
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                warn!("Could not evict cover art {}: {}", coverid, e);
            }
        }
    }

    /// Number of JPEG files currently in the cache directory.
    pub fn cached_count(&self) -> usize {
        std::fs::read_dir(&self.cache_dir)
            .map(|d| d.filter_map(|e| e.ok()).count())
            .unwrap_or(0)
    }

    // ── Private ───────────────────────────────────────────────────────────────

    /// Absolute path for a given `coverid` cache entry.
    fn cache_path(&self, coverid: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.jpg", coverid))
    }

    /// Read a cached JPEG from `path` and decode to Rgb565.
    fn load_from_cache(&self, path: &Path) -> Result<CoverArt, CoverArtError> {
        let jpeg = std::fs::read(path)?;
        let img = image::load_from_memory_with_format(&jpeg, ImageFormat::Jpeg)?;
        Ok(CoverArt::from_image(img))
    }

    /// Fetch raw JPEG bytes from `url`. Returns `NotAvailable` for empty responses.
    async fn fetch_jpeg(&self, url: &str) -> Result<Vec<u8>, CoverArtError> {
        debug!("GET {}", url);
        let bytes = self.client.get(url).send().await?.bytes().await?;
        if bytes.is_empty() {
            return Err(CoverArtError::NotAvailable);
        }
        Ok(bytes.to_vec())
    }

    /// Decode `jpeg`, resize to COVER_SIZE², save to `cache_path`, return Rgb565.
    fn decode_resize_cache(
        &self,
        coverid: &str,
        jpeg: &[u8],
        cache_path: &Path,
    ) -> Result<CoverArt, CoverArtError> {
        // Decode
        let img = image::load_from_memory(jpeg)?;

        // Resize to COVER_SIZE × COVER_SIZE (Lanczos3 for quality)
        let resized = img.resize_exact(COVER_SIZE, COVER_SIZE, image::imageops::FilterType::Lanczos3);

        // Save to cache as JPEG at controlled quality
        let file = std::fs::File::create(cache_path)?;
        let mut enc = JpegEncoder::new_with_quality(BufWriter::new(file), CACHE_JPEG_QUALITY);
        enc.encode_image(&resized)?;
        debug!("Cached cover art: {}.jpg ({} bytes)", coverid, jpeg.len());

        Ok(CoverArt::from_image(resized))
    }
}
