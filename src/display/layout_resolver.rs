/*
 *  display/layout_resolver.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Resolves a named template + display profile into a concrete `PageLayout`.
 *
 *  Steps:
 *    1. Find the template by name in `LayoutTemplates`.
 *    2. Walk its variants; pick the first whose `MatchRule` matches the profile.
 *    3. For each `Region` in the chosen variant:
 *         a. Evaluate the region's x/y/w/h expressions (display.* vars only).
 *         b. Instantiate the named component at that bounding box:
 *              - Walk component fields in declaration order.
 *              - Evaluate each field's x/y/w/h relative to the region origin
 *                and using accumulated field geometry.
 *              - Build a `Field` with concrete `Rectangle`.
 *    4. Collect all fields from all regions → `PageLayout`.
 *
 *  The resulting `PageLayout` is the same type as those produced by the legacy
 *  `LayoutManager::create_*` functions, so no changes to the renderer are
 *  required at this stage.
 *
 *  This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 */

#![allow(dead_code)]

use std::collections::HashMap;
use embedded_graphics::prelude::{Point, Size};
use embedded_graphics::primitives::Rectangle;

use crate::display::{
    field::{Field, FieldType},
    page::PageLayout,
    layout_template::{
        CategoryFilter, ColorDepthFilter, FieldKind, LayoutTemplates, MatchRule,
    },
    layout_expr::{eval, ExprContext, FieldGeom},
};
use crate::display::traits::ColorDepth;
use crate::display::layout::LayoutCategory;

/// Characteristics of the target display, used for variant matching and
/// expression evaluation.
#[derive(Debug, Clone, Copy)]
pub struct DisplayProfile {
    pub width:    u32,
    pub height:   u32,
    pub color_depth: ColorDepth,
    pub category: LayoutCategory,
}

/// Resolves a named template for a given display profile.
pub struct LayoutResolver<'t> {
    templates: &'t LayoutTemplates,
}

impl<'t> LayoutResolver<'t> {
    pub fn new(templates: &'t LayoutTemplates) -> Self {
        Self { templates }
    }

    /// Resolve `template_name` for `profile` into a concrete `PageLayout`.
    ///
    /// Returns `None` if the template is unknown or no variant matches.
    pub fn resolve(&self, template_name: &str, profile: DisplayProfile) -> Option<PageLayout> {
        let template = self.templates.templates.get(template_name)?;

        // Pick first matching variant
        let variant = template.variants.iter().find(|v| {
            v.match_rule.is_catch_all() || matches_profile(&v.match_rule, profile)
        })?;

        let dw = profile.width  as i32;
        let dh = profile.height as i32;

        let mut all_fields: Vec<Field> = Vec::new();

        for region in &variant.regions {
            // Resolve region bounds (display.* only — no field refs at this level)
            let empty_fields: HashMap<String, FieldGeom> = HashMap::new();
            let region_ctx = ExprContext {
                display_width:  dw,
                display_height: dh,
                parent_width:   dw,
                parent_height:  dh,
                fields: &empty_fields,
                font_height: 0,
            };

            let rx = eval(&region.x,      &region_ctx).unwrap_or(0);
            let ry = eval(&region.y,      &region_ctx).unwrap_or(0);
            let rw = eval(&region.width,  &region_ctx).unwrap_or(dw);
            let rh = eval(&region.height, &region_ctx).unwrap_or(dh);

            // Instantiate component fields within region bounds
            let component = match self.templates.components.get(&region.component) {
                Some(c) => c,
                None => {
                    log::warn!("layout_resolver: unknown component '{}'", region.component);
                    continue;
                }
            };

            let mut field_geoms: HashMap<String, FieldGeom> = HashMap::new();

            for field_def in &component.fields {
                let font_h = field_def.font.as_ref()
                    .map(|f| f.to_mono_font().character_size.height as i32)
                    .unwrap_or(0);

                let field_ctx = ExprContext {
                    display_width:  dw,
                    display_height: dh,
                    parent_width:   rw,
                    parent_height:  rh,
                    fields: &field_geoms,
                    font_height: font_h,
                };

                let fx = eval(&field_def.x,      &field_ctx).unwrap_or(0);
                let fy = eval(&field_def.y,      &field_ctx).unwrap_or(0);
                let fw = eval(&field_def.width,  &field_ctx).unwrap_or(rw);
                let fh = eval(&field_def.height, &field_ctx).unwrap_or(0);

                // Clamp to non-negative sizes
                let fw = fw.max(0) as u32;
                let fh = fh.max(0) as u32;

                // Offset by region origin
                let abs_x = rx + fx;
                let abs_y = ry + fy;

                let bounds = Rectangle::new(
                    Point::new(abs_x, abs_y),
                    Size::new(fw, fh),
                );

                // Accumulate geometry for subsequent field expressions
                field_geoms.insert(field_def.name.clone(), FieldGeom {
                    x: fx, y: fy, w: fw as i32, h: fh as i32,
                });

                // Build Field
                let field_type = kind_to_field_type(&field_def.field_type);
                let fg = crate::display::color::Color::from(&field_def.fg_color);
                let bg = field_def.bg_color.as_ref().map(crate::display::color::Color::from);
                let font = field_def.font.as_ref().map(|f| f.to_mono_font());

                let field = Field {
                    name:                 field_def.name.clone(),
                    field_type,
                    bounds,
                    border:               field_def.border,
                    scrollable:           field_def.scrollable,
                    font,
                    fg_color:             fg,
                    bg_color:             bg,
                    horizontal_alignment: field_def.horizontal_alignment.clone().into(),
                    vertical_alignment:   field_def.vertical_alignment.clone().into(),
                };

                all_fields.push(field);
            }
        }

        // Process inline fields (display dims serve as both display and parent)
        if !variant.fields.is_empty() {
            let mut field_geoms: HashMap<String, FieldGeom> = HashMap::new();
            for field_def in &variant.fields {
                let font_h = field_def.font.as_ref()
                    .map(|f| f.to_mono_font().character_size.height as i32)
                    .unwrap_or(0);

                let field_ctx = ExprContext {
                    display_width:  dw,
                    display_height: dh,
                    parent_width:   dw,
                    parent_height:  dh,
                    fields: &field_geoms,
                    font_height: font_h,
                };

                let fx = eval(&field_def.x,      &field_ctx).unwrap_or(0);
                let fy = eval(&field_def.y,      &field_ctx).unwrap_or(0);
                let fw = eval(&field_def.width,  &field_ctx).unwrap_or(dw).max(0) as u32;
                let fh = eval(&field_def.height, &field_ctx).unwrap_or(0).max(0) as u32;

                field_geoms.insert(field_def.name.clone(), FieldGeom {
                    x: fx, y: fy, w: fw as i32, h: fh as i32,
                });

                let bounds = Rectangle::new(Point::new(fx, fy), Size::new(fw, fh));
                let field_type = kind_to_field_type(&field_def.field_type);
                let fg = crate::display::color::Color::from(&field_def.fg_color);
                let bg = field_def.bg_color.as_ref().map(crate::display::color::Color::from);
                let font = field_def.font.as_ref().map(|f| f.to_mono_font());

                all_fields.push(Field {
                    name:                 field_def.name.clone(),
                    field_type,
                    bounds,
                    border:               field_def.border,
                    scrollable:           field_def.scrollable,
                    font,
                    fg_color:             fg,
                    bg_color:             bg,
                    horizontal_alignment: field_def.horizontal_alignment.clone().into(),
                    vertical_alignment:   field_def.vertical_alignment.clone().into(),
                });
            }
        }

        Some(PageLayout::new(format!("{}:{}", template_name, variant.name))
            .add_fields(all_fields))
    }
}

fn matches_profile(rule: &MatchRule, profile: DisplayProfile) -> bool {
    if !rule.category.is_empty() && !rule.category.iter().any(|c| category_matches(c, profile.category)) {
        return false;
    }
    if !rule.color_depth.is_empty() && !rule.color_depth.iter().any(|d| depth_matches(d, profile.color_depth)) {
        return false;
    }
    true
}

fn category_matches(filter: &CategoryFilter, category: LayoutCategory) -> bool {
    match (filter, category) {
        (CategoryFilter::Small,      LayoutCategory::Small)      => true,
        (CategoryFilter::Medium,     LayoutCategory::Medium)     => true,
        (CategoryFilter::Large,      LayoutCategory::Large)      => true,
        (CategoryFilter::ExtraLarge, LayoutCategory::ExtraLarge) => true,
        _ => false,
    }
}

fn depth_matches(filter: &ColorDepthFilter, depth: ColorDepth) -> bool {
    match (filter, depth) {
        (ColorDepthFilter::Monochrome, ColorDepth::Monochrome) => true,
        (ColorDepthFilter::Gray4,      ColorDepth::Gray4)      => true,
        (ColorDepthFilter::Rgb565,     ColorDepth::Rgb565)     => true,
        _ => false,
    }
}

fn kind_to_field_type(kind: &FieldKind) -> FieldType {
    match kind {
        FieldKind::WeatherIcon | FieldKind::WeatherGlyph | FieldKind::CoverImage => FieldType::Glyph,

        FieldKind::Custom
        | FieldKind::TrackProgressBar
        | FieldKind::SecondsProgress
        | FieldKind::ClockDigits => FieldType::Custom,

        _ => FieldType::Text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::traits::ColorDepth;
    use crate::display::layout::LayoutCategory;

    const MINIMAL_YAML: &str = r#"
components:
  test_panel:
    fields:
      - name: header
        type: label
        x: "0"
        y: "0"
        width: "parent.width"
        height: "9"
        fg_color: White

      - name: body
        type: scrolling_text
        x: "0"
        y: "header.bottom + 1"
        width: "parent.width"
        height: "9"
        fg_color: White

templates:
  test_page:
    variants:
      - name: default
        regions:
          - component: test_panel
            x: "0"
            y: "0"
            width: "display.width"
            height: "display.height"
"#;

    fn profile_128() -> DisplayProfile {
        DisplayProfile {
            width: 128,
            height: 64,
            color_depth: ColorDepth::Monochrome,
            category: LayoutCategory::Small,
        }
    }

    #[test]
    fn resolves_basic_template() {
        let templates = LayoutTemplates::from_yaml(MINIMAL_YAML).unwrap();
        let resolver = LayoutResolver::new(&templates);
        let page = resolver.resolve("test_page", profile_128()).unwrap();

        let fields = page.fields();
        assert_eq!(fields.len(), 2);

        let header = &fields[0];
        assert_eq!(header.name, "header");
        assert_eq!(header.bounds.top_left, Point::new(0, 0));
        assert_eq!(header.bounds.size, Size::new(128, 9));

        let body = &fields[1];
        assert_eq!(body.name, "body");
        // y = header.bottom + 1 = 9 + 1 = 10
        assert_eq!(body.bounds.top_left, Point::new(0, 10));
    }

    #[test]
    fn unknown_template_returns_none() {
        let templates = LayoutTemplates::from_yaml(MINIMAL_YAML).unwrap();
        let resolver = LayoutResolver::new(&templates);
        assert!(resolver.resolve("no_such_page", profile_128()).is_none());
    }

    #[test]
    fn variant_matching_by_category() {
        const YAML: &str = r#"
components:
  panel:
    fields:
      - name: f
        type: label
        x: "0"
        y: "0"
        width: "parent.width"
        height: "9"

templates:
  page:
    variants:
      - name: wide
        match:
          category: [Large]
        regions:
          - component: panel
            x: "0"
            y: "0"
            width: "display.width / 2"
            height: "display.height"
      - name: narrow
        regions:
          - component: panel
            x: "0"
            y: "0"
            width: "display.width"
            height: "display.height"
"#;
        let templates = LayoutTemplates::from_yaml(YAML).unwrap();
        let resolver = LayoutResolver::new(&templates);

        let narrow = resolver.resolve("page", profile_128()).unwrap();
        assert_eq!(narrow.fields()[0].bounds.size.width, 128); // full width

        let wide_profile = DisplayProfile {
            width: 256, height: 64,
            color_depth: ColorDepth::Gray4,
            category: LayoutCategory::Large,
        };
        let wide = resolver.resolve("page", wide_profile).unwrap();
        assert_eq!(wide.fields()[0].bounds.size.width, 128); // half of 256
    }

    #[test]
    fn rgb565_variant_matching() {
        const YAML: &str = r#"
components:
  panel:
    fields:
      - name: f
        type: label
        x: "0"
        y: "0"
        width: "parent.width"
        height: "9"

templates:
  page:
    variants:
      - name: color
        match:
          color_depth: [Rgb565]
        regions:
          - component: panel
            x: "display.height"
            y: "0"
            width: "display.width - display.height"
            height: "display.height"
      - name: mono
        regions:
          - component: panel
            x: "0"
            y: "0"
            width: "display.width"
            height: "display.height"
"#;
        let templates = LayoutTemplates::from_yaml(YAML).unwrap();
        let resolver = LayoutResolver::new(&templates);

        let rgb_profile = DisplayProfile {
            width: 320, height: 170,
            color_depth: ColorDepth::Rgb565,
            category: LayoutCategory::ExtraLarge,
        };
        let page = resolver.resolve("page", rgb_profile).unwrap();
        // x = display.height = 170, width = 320 - 170 = 150
        assert_eq!(page.fields()[0].bounds.top_left.x, 170);
        assert_eq!(page.fields()[0].bounds.size.width, 150);
    }
}
