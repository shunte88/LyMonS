/*
 *  display/layout_template.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Declarative layout template schema.
 *
 *  Defines the serde types that map to `layout.yaml`.  These are pure
 *  data — no rendering logic lives here.
 *
 *  Hierarchy:
 *    LayoutTemplates            — root, loaded once at startup
 *      components: HashMap<name, ComponentDef>
 *      templates:  HashMap<name, TemplateDef>
 *
 *    ComponentDef               — reusable group of fields
 *      fields: Vec<FieldDef>   — positions expressed as Expr strings
 *
 *    TemplateDef                — page definition
 *      variants: Vec<Variant>  — ordered; first match wins
 *        match: MatchRule      — filter on display characteristics
 *        regions: Vec<Region>  — component placements in the page
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 */

#![allow(dead_code)]

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Scan a YAML text to find which top-level section entry (component or template
/// name) contains the given 1-based line number.
///
/// Top-level section headers (`components:`, `templates:`) are at column 0.
/// Entry names are at exactly 2-space indent (`  egg_bass:`).
/// Returns a string like `component 'egg_bass'` or `template 'easter_egg_vcr'`.
fn yaml_entry_at_line(yaml: &str, error_line: usize) -> Option<String> {
    let mut section = "";
    let mut entry   = "";

    for (idx, line) in yaml.lines().enumerate() {
        let lineno = idx + 1;
        if lineno > error_line { break; }

        if line.starts_with("components:") {
            section = "component";
            entry   = "";
        } else if line.starts_with("templates:") {
            section = "template";
            entry   = "";
        } else if line.starts_with("  ") && !line.starts_with("   ") {
            // Exactly 2-space indent — a top-level entry name
            if let Some(key) = line.trim_end().strip_suffix(':') {
                entry = key.trim();
            }
        }
    }

    if !section.is_empty() && !entry.is_empty() {
        Some(format!("{section} '{entry}'"))
    } else {
        None
    }
}

/// Root type — deserialised from `layout.yaml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LayoutTemplates {
    /// Reusable field groups, keyed by name (e.g. "scroller", "clock_face").
    #[serde(default)]
    pub components: HashMap<String, ComponentDef>,

    /// Page templates, keyed by name (e.g. "playback", "clock", "aio").
    #[serde(default)]
    pub templates: HashMap<String, TemplateDef>,
}

impl LayoutTemplates {
    /// Load from a YAML string (use `include_str!` for embedded, or read file
    /// for runtime override).
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Load the embedded base layout (assets/layout.yaml, 128×64 mono geometry).
    pub fn default_layout() -> Result<Self, serde_yaml::Error> {
        Self::from_yaml(include_str!("../../assets/layout.yaml"))
    }

    /// Merge `other` into `self`.
    ///
    /// **Components** — full replacement: any component in `other` replaces the
    /// same-named component in `self`; new names are added.
    ///
    /// **Templates** — field-level merge within each matching variant:
    ///   - If a template name exists in both, variants are matched by name.
    ///   - Within a matched variant, each field in `other` is upserted by name:
    ///     existing fields are replaced, new fields are appended.  Base-only fields
    ///     survive unchanged.  This lets a driver override add a single field (e.g.
    ///     add `artist` to `easter_egg_pipboy`) without having to redeclare all the
    ///     other fields from the base template.
    ///   - Variants present in `other` but not in `self` are appended wholesale.
    ///   - Template names that appear only in `other` are added as-is.
    pub fn merge(&mut self, other: LayoutTemplates) {
        self.components.extend(other.components);

        for (name, other_tmpl) in other.templates {
            match self.templates.get_mut(&name) {
                None => { self.templates.insert(name, other_tmpl); }
                Some(base_tmpl) => {
                    for other_variant in other_tmpl.variants {
                        match base_tmpl.variants.iter_mut().find(|v| v.name == other_variant.name) {
                            None => base_tmpl.variants.push(other_variant),
                            Some(base_variant) => {
                                // Upsert each field by name
                                for other_field in other_variant.fields {
                                    match base_variant.fields.iter_mut().find(|f| f.name == other_field.name) {
                                        Some(base_field) => *base_field = other_field,
                                        None => base_variant.fields.push(other_field),
                                    }
                                }
                                // Regions in the override also replace their base counterparts
                                for other_region in other_variant.regions {
                                    match base_variant.regions.iter_mut().find(|r| r.component == other_region.component) {
                                        Some(base_region) => *base_region = other_region,
                                        None => base_variant.regions.push(other_region),
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Load the base layout and, if `{asset_path}layout.yaml` exists on disk,
    /// merge the driver-specific override on top.
    ///
    /// `asset_path` should be the same value as `LayoutConfig::asset_path`
    /// (e.g. `"./assets/ssd1322/"`).  Errors parsing the override are logged
    /// and ignored so the base layout is always returned.
    pub fn load_with_driver_override(asset_path: &str) -> Self {
        let mut base = Self::default_layout().unwrap_or_else(|e| {
            log::error!("layout: failed to parse embedded assets/layout.yaml: {e}");
            Self { components: HashMap::new(), templates: HashMap::new() }
        });

        let override_path = format!("{asset_path}layout.yaml");
        match std::fs::read_to_string(&override_path) {
            Ok(yaml) => match Self::from_yaml(&yaml) {
                Ok(over) => {
                    log::info!("layout: loaded driver override {override_path}");
                    base.merge(over);
                }
                Err(e) => {
                    log::error!("layout: YAML error in {override_path}:");
                    for line in e.to_string().lines() {
                        log::error!("  {line}");
                    }
                    // Add structural context: scan the raw text to find which
                    // component or template the error line falls inside.
                    if let Some(loc) = e.location() {
                        if let Some(ctx) = yaml_entry_at_line(&yaml, loc.line()) {
                            log::error!("  (in {ctx})");
                        }
                    }
                    log::warn!("layout: driver override ignored — using base layout only");
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // No driver override — use base layout unchanged.
            }
            Err(e) => log::warn!("layout: could not read {override_path}: {e}"),
        }

        base
    }
}

/// A named, reusable group of fields.
///
/// All field positions are relative to the component's own origin (0,0).
/// The component is sized by the `Region` that places it.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ComponentDef {
    pub fields: Vec<FieldDef>,
}

/// A single UI field within a component.
///
/// Position and size values are expression strings evaluated at
/// instantiation time by `layout_expr::eval`.
///
/// Available variables (resolved left-to-right, so each field may reference
/// any field defined above it in the same component):
///
///   `display.width`  `display.height`
///   `parent.width`   `parent.height`
///   `<name>.top`     `<name>.bottom`  `<name>.left`  `<name>.right`
///   `<name>.width`   `<name>.height`
///   `font_height`    (resolved from the `font` property when set)
///
/// Arithmetic: `+`, `-`, `*`, `/` over integer values.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FieldDef {
    /// Identifier used by the renderer (e.g. "status_bar", "album_artist").
    pub name: String,

    /// Semantic field kind — drives renderer dispatch.
    #[serde(rename = "type")]
    pub field_type: FieldKind,

    /// X position expression (default `"0"`).
    #[serde(default = "zero_str")]
    pub x: String,

    /// Y position expression (default `"0"`).
    #[serde(default = "zero_str")]
    pub y: String,

    /// Width expression (default `"parent.width"`).
    #[serde(default = "parent_width_str")]
    pub width: String,

    /// Height expression.  Required unless `field_type` provides a natural
    /// size (e.g. `clock_digits` derives height from font metrics).
    #[serde(default = "zero_str")]
    pub height: String,

    /// Foreground color (default `White`).
    #[serde(default = "default_fg")]
    pub fg_color: ColorSpec,

    /// Background color (default transparent / None).
    #[serde(default)]
    pub bg_color: Option<ColorSpec>,

    /// Font name override.  When omitted the field type provides a default.
    #[serde(default)]
    pub font: Option<FontSpec>,

    /// Border stroke width in pixels (default 0 = no border).
    #[serde(default)]
    pub border: u8,

    /// Whether content scrolls horizontally when it overflows (default false).
    #[serde(default)]
    pub scrollable: bool,

    /// Horizontal text alignment (default Left).
    #[serde(default)]
    pub horizontal_alignment: AlignH,

    /// Vertical text alignment (default Top).
    #[serde(default)]
    pub vertical_alignment: AlignV,
}

fn zero_str() -> String { "0".to_string() }
fn parent_width_str() -> String { "parent.width".to_string() }
fn default_fg() -> ColorSpec { ColorSpec::Named(NamedColor::White) }

/// Semantic field type — determines which renderer handles the field.
///
/// All standard types are handled generically in `manager.rs` dispatch.
/// `Custom` is an escape hatch for fields that need bespoke drawing
/// (e.g. moon phase glyph, weather glyph).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    StatusBar,
    StatusBarSmall,
    ScrollingText,
    ScrollingTextCombination,
    TrackProgressBar,
    InfoLine,
    ClockDigits,
    SecondsProgress,
    Date,
    WeatherIcon,
    WeatherGlyph,
    WeatherText,
    CoverImage,
    Label,
    /// Bespoke rendering identified by `name`; handled by existing string-match
    /// arms in the dispatcher.  Use sparingly.
    Custom,
}

/// A page template — a set of variants tried in order; first match wins.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TemplateDef {
    pub variants: Vec<Variant>,
}

/// One display-specific variant of a template.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Variant {
    /// Human-readable name for diagnostics.
    pub name: String,

    /// Display characteristics that must match for this variant to apply.
    /// `None` fields are wildcards.  An entirely absent `match` block is a
    /// catch-all (use as the last variant).
    #[serde(rename = "match", default)]
    pub match_rule: MatchRule,

    /// Component placements that make up this page.
    #[serde(default)]
    pub regions: Vec<Region>,

    /// Inline field definitions — an alternative to `regions` for simple
    /// layouts that don't need a reusable component.  Fields here are
    /// resolved with `display.*` as both display and parent dimensions,
    /// and may reference each other by name (same rules as component fields).
    /// Both `regions` and `fields` may be present; regions are processed first.
    #[serde(default)]
    pub fields: Vec<FieldDef>,
}

/// Display characteristic filters.  Each field is `None` → wildcard.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MatchRule {
    /// Restrict to specific layout categories.
    #[serde(default)]
    pub category: Vec<CategoryFilter>,

    /// Restrict to specific color depths.
    #[serde(default)]
    pub color_depth: Vec<ColorDepthFilter>,
}

impl MatchRule {
    pub fn is_catch_all(&self) -> bool {
        self.category.is_empty() && self.color_depth.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum CategoryFilter {
    Small,
    Medium,
    Large,
    ExtraLarge,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum ColorDepthFilter {
    Monochrome,
    Gray4,
    Rgb565,
}

/// A component instance placed inside a template variant.
///
/// `x`, `y`, `width`, `height` are expression strings resolved against
/// `display.*` variables only (no prior-field references at the template level).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Region {
    /// Name of the component to instantiate (key in `LayoutTemplates::components`).
    pub component: String,

    /// X offset of the component within the display.
    #[serde(default = "zero_str")]
    pub x: String,

    /// Y offset of the component within the display.
    #[serde(default = "zero_str")]
    pub y: String,

    /// Width allocated to the component.
    #[serde(default = "display_width_str")]
    pub width: String,

    /// Height allocated to the component.
    #[serde(default = "display_height_str")]
    pub height: String,
}

fn display_width_str()  -> String { "display.width".to_string() }
fn display_height_str() -> String { "display.height".to_string() }

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ColorSpec {
    Named(NamedColor),
    Rgb { r: u8, g: u8, b: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum NamedColor {
    White,
    Black,
    Cyan,
    Yellow,
    Red,
    Blue,
    Green,
    Orange,
    Magenta,
}

impl From<&ColorSpec> for crate::display::color::Color {
    fn from(c: &ColorSpec) -> Self {
        use crate::display::color::Color;
        match c {
            ColorSpec::Named(n) => match n {
                NamedColor::White   => Color::White,
                NamedColor::Black   => Color::Black,
                NamedColor::Cyan    => Color::Cyan,
                NamedColor::Yellow  => Color::Yellow,
                NamedColor::Red     => Color::Red,
                NamedColor::Blue    => Color::Blue,
                NamedColor::Green   => Color::Green,
                NamedColor::Orange  => Color::Orange,
                NamedColor::Magenta => Color::Magenta,
            },
            ColorSpec::Rgb { r, g, b } => Color::Rgb(*r, *g, *b),
        }
    }
}

/// Font identifier.  Resolved to a `&'static MonoFont` by the resolver.
///
/// Explicit renames are required because serde's `snake_case` does not insert
/// an underscore before digit groups, producing e.g. `font4x6` instead of
/// the intended `font_4x6`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum FontSpec {
    #[serde(rename = "font_4x6")]    Font4x6,
    #[serde(rename = "font_5x7")]    Font5x7,
    #[serde(rename = "font_5x8")]    Font5x8,
    #[serde(rename = "font_6x9")]    Font6x9,
    #[serde(rename = "font_6x10")]   Font6x10,
    #[serde(rename = "font_6x13_bold")] Font6x13Bold,
    #[serde(rename = "font_7x13")]   Font7x13,
    #[serde(rename = "font_7x13_bold")] Font7x13Bold,
    #[serde(rename = "font_7x14")]   Font7x14,
    #[serde(rename = "font_10x20")]   Font10x20,
}

impl FontSpec {
    pub fn to_mono_font(&self) -> &'static embedded_graphics::mono_font::MonoFont<'static> {
        use embedded_graphics::mono_font::iso_8859_13::*;
        match self {
            FontSpec::Font4x6      => &FONT_4X6,
            FontSpec::Font5x7      => &FONT_5X7,
            FontSpec::Font5x8      => &FONT_5X8,
            FontSpec::Font6x9      => &FONT_6X9,
            FontSpec::Font6x10     => &FONT_6X10,
            FontSpec::Font6x13Bold => &FONT_6X13_BOLD,
            FontSpec::Font7x13     => &FONT_7X13,
            FontSpec::Font7x13Bold => &FONT_7X13_BOLD,
            FontSpec::Font7x14     => &FONT_7X14,
            FontSpec::Font10x20     => &FONT_10X20,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum AlignH {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum AlignV {
    #[default]
    Top,
    Middle,
    Bottom,
}

impl From<AlignH> for embedded_text::alignment::HorizontalAlignment {
    fn from(a: AlignH) -> Self {
        match a {
            AlignH::Left   => embedded_text::alignment::HorizontalAlignment::Left,
            AlignH::Center => embedded_text::alignment::HorizontalAlignment::Center,
            AlignH::Right  => embedded_text::alignment::HorizontalAlignment::Right,
        }
    }
}

impl From<AlignV> for embedded_text::alignment::VerticalAlignment {
    fn from(a: AlignV) -> Self {
        match a {
            AlignV::Top    => embedded_text::alignment::VerticalAlignment::Top,
            AlignV::Middle => embedded_text::alignment::VerticalAlignment::Middle,
            AlignV::Bottom => embedded_text::alignment::VerticalAlignment::Bottom,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_layout_parses() {
        let t = LayoutTemplates::default_layout()
            .expect("assets/layout.yaml must parse without errors");
        assert!(t.components.contains_key("scroller_panel"));
        assert!(t.components.contains_key("aio_compact_panel"));
        assert!(t.components.contains_key("clock_face_small"));
        assert!(t.components.contains_key("clock_face_large"));
        assert!(t.components.contains_key("splash_screen"));
        assert!(t.components.contains_key("weather_current_main"));
        assert!(t.components.contains_key("weather_astral_panel"));
        assert!(t.components.contains_key("weather_forecast_3col"));
        assert!(t.components.contains_key("weather_forecast_ext_cols"));
        assert!(t.templates.contains_key("playback"));
        assert!(t.templates.contains_key("aio"));
        assert!(t.templates.contains_key("clock"));
        assert!(t.templates.contains_key("splash"));
        assert!(t.templates.contains_key("weather_current"));
        assert!(t.templates.contains_key("weather_forecast"));
    }
}
