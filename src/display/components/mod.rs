/*
 *  display/components/mod.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  UI components for display rendering
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

pub mod status_bar;
pub mod scrollers;
pub mod clock;
pub mod weather;
pub mod visualizer;
pub mod easter_eggs;

// Re-exports
pub use status_bar::StatusBar;
pub use scrollers::ScrollingText;
pub use clock::ClockDisplay;
pub use weather::WeatherDisplay;
pub use visualizer::VisualizerComponent;
pub use easter_eggs::EasterEggsComponent;
