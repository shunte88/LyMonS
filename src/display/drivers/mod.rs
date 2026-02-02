/*
 *  display/drivers/mod.rs
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Display driver implementations
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

// Conditionally compile each driver based on feature flags
#[cfg(feature = "driver-ssd1306")]
pub mod ssd1306;

#[cfg(feature = "driver-ssd1309")]
pub mod ssd1309;

#[cfg(feature = "driver-ssd1322")]
pub mod ssd1322;

#[cfg(feature = "driver-sh1106")]
pub mod sh1106;

// Mock driver for testing
#[cfg(test)]
pub mod mock;

// Emulator driver for desktop testing
#[cfg(feature = "emulator")]
pub mod emulator;
