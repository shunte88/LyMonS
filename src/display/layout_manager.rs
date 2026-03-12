/*
 *  display/layout_manager.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Layout manager - owns all page definitions for consistent display
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

#![allow(dead_code)] // layout manager helpers; some page-builder fns reserved

use super::page::PageLayout;
use super::layout::{LayoutConfig, LayoutCategory};
use super::layout_template::LayoutTemplates;
use super::layout_resolver::{DisplayProfile, LayoutResolver};

pub const SCROLLING_PAGE: &str = "scrolling";
pub const SCROLLING_AIO_PAGE: &str = "aio_small";
pub const SCROLLING_AIO_WIDE_PAGE: &str = "aio_wide";

pub struct LayoutManager {
    layout_config: LayoutConfig,
    templates: LayoutTemplates,
    profile: DisplayProfile,
}

impl LayoutManager {

    pub fn new(layout_config: LayoutConfig) -> Self {
        let templates = LayoutTemplates::load_with_driver_override(&layout_config.asset_path);
        let profile = DisplayProfile {
            width:       layout_config.width,
            height:      layout_config.height,
            color_depth: layout_config.color_depth,
            category:    layout_config.category,
        };
        Self { layout_config, templates, profile }
    }

    fn resolve(&self, template_name: &str) -> Option<PageLayout> {
        LayoutResolver::new(&self.templates).resolve(template_name, self.profile)
    }

    pub fn create_aio_scrolling_page(&self) -> PageLayout {
        self.resolve("aio").unwrap_or_else(|| {
            log::error!("layout_manager: failed to resolve 'aio' template");
            PageLayout::new(SCROLLING_AIO_PAGE)
        })
    }

    pub fn create_scrolling_page(&self, page_name: &str) -> PageLayout {
        let template = if page_name == SCROLLING_PAGE { "playback" } else { "aio" };
        self.resolve(template).unwrap_or_else(|| {
            log::error!("layout_manager: failed to resolve '{template}' template");
            PageLayout::new(page_name)
        })
    }

    pub fn create_clock_page(&self) -> PageLayout {
        self.resolve("clock").unwrap_or_else(|| {
            log::error!("layout_manager: failed to resolve 'clock' template");
            PageLayout::new("clock")
        })
    }

    pub fn is_wide(&self) -> bool{
        matches!(self.layout_config.category, LayoutCategory::Large | LayoutCategory::ExtraLarge)
    }

    pub fn create_weather_current_page(&self) -> PageLayout {
        self.resolve("weather_current").unwrap_or_else(|| {
            log::error!("layout_manager: failed to resolve 'weather_current' template");
            PageLayout::new("weather_current")
        })
    }

    pub fn create_weather_forecast_page(&self) -> PageLayout {
        self.resolve("weather_forecast").unwrap_or_else(|| {
            log::error!("layout_manager: failed to resolve 'weather_forecast' template");
            PageLayout::new("weather_forecast")
        })
    }

    pub fn create_warning_page(&self) -> PageLayout {
        self.resolve("warning").unwrap_or_else(|| {
            log::error!("layout_manager: failed to resolve 'warning' template");
            PageLayout::new("warning")
        })
    }

    pub fn create_splash_page(&self) -> PageLayout {
        self.resolve("splash").unwrap_or_else(|| {
            log::error!("layout_manager: failed to resolve 'splash' template");
            PageLayout::new("splash")
        })
    }

    /// Get the layout configuration
    pub fn layout_config(&self) -> &LayoutConfig {
        &self.layout_config
    }
}
