/*
 *  lymons-gpio
 *
 *  LyMonS - worth the squeeze
 *  (c) 2020-26 Stuart Hunter
 *
 *  Shared gpiochip selection for the SPI display drivers.
 *
 *  Every 4-wire SPI panel needs two GPIO lines (DC, and optionally RST).  How
 *  you name those lines depends entirely on the board:
 *
 *  Raspberry Pi
 *      One controller covers the whole 40-pin header, and its cdev line offset
 *      happens to equal the BCM number — so "DC on BCM 24" is literally line 24
 *      on the header chip.  The *chip number* has moved around between models
 *      and kernels (Pi 5 was gpiochip4, then gpiochip0 on 6.6+), so we find it
 *      by the controller's stable label instead of a hardcoded path.
 *
 *  Orange Pi / Rockchip (RK3566, RK3588, ...)
 *      gpio-rockchip registers one controller *per bank* — gpio0 … gpio4, each
 *      exposing 32 lines — so there is no single "header chip" to detect and no
 *      global numbering.  A pin such as PC7 on bank 3 is bank 3, line 23:
 *
 *          line = group * 8 + index      (group A=0, B=1, C=2, D=3)
 *          PC7  = 2 * 8 + 7 = 23         on chip "gpio3"
 *
 *      Pick the bank with `gpio_chip`, then give the bank-relative line as
 *      `dc_pin` / `rst_pin`.
 *
 *  Allwinner (Orange Pi Zero 3 / Zero 2W on H618, older Zero / PC / One on H3)
 *      Two controllers: the main pinctrl and the R_PIO that carries the PL bank.
 *      Within each, line numbers are flat across banks:
 *
 *          line = bank * 32 + index      (bank A=0, B=1, C=2 … I=8)
 *          PC7  = 2 * 32 + 7 = 71        on the main pinctrl
 *          PL10 = 10                     on the R_PIO controller
 *
 *      The labels are device-tree node names and differ per SoC ("300b000.pinctrl"
 *      / "7022000.pinctrl" on H616/H618, "1c20800.pinctrl" / "1f02c00.pinctrl" on
 *      H3), so read them off the board rather than assuming — the fallback path
 *      below logs every controller it finds.
 *
 *  Selection order:
 *      1. explicit selector (config `display.bus.gpio_chip`)
 *      2. the LYMONS_GPIO_CHIP environment variable
 *      3. Raspberry Pi header autodetection by controller label
 *      4. DEFAULT_GPIO_CHIP, with the available controllers logged so a non-Pi
 *         user can see what to put in `gpio_chip`
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
 */

use std::fmt;

use linux_embedded_hal::CdevPin;
use linux_embedded_hal::gpio_cdev::{self, Chip, LineRequestFlags};
use log::{info, warn};

/// Used when nothing else identifies the controller.
pub const DEFAULT_GPIO_CHIP: &str = "/dev/gpiochip0";

/// Environment override, mainly for plugin drivers which do not see the
/// application's YAML config.
pub const CHIP_ENV_VAR: &str = "LYMONS_GPIO_CHIP";

/// Raspberry Pi 40-pin header controllers, most specific (newest) first.
const HEADER_LABELS: [&str; 4] = [
    "pinctrl-rp1",     // Pi 5
    "pinctrl-bcm2711", // Pi 4 / CM4
    "pinctrl-bcm2835", // Pi 0/1/2/3 / Zero
    "pinctrl-bcm2708", // very old kernels
];

/// A GPIO failure, carrying a message the caller wraps in its own error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpioError(String);

impl GpioError {
    pub fn message(&self) -> &str {
        &self.0
    }
    pub fn into_message(self) -> String {
        self.0
    }
}

impl fmt::Display for GpioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GpioError {}

/// How a user-supplied `gpio_chip` string names a controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChipSelector {
    /// A device node — `/dev/gpiochip3`.
    Path(String),
    /// A chip number — `3` or `gpiochip3`.
    Number(u32),
    /// A controller label — `gpio3` (Rockchip), `pinctrl-rp1` (Pi 5),
    /// `1c20800.pinctrl` (Allwinner).
    Label(String),
}

/// Classify a `gpio_chip` selector without touching the filesystem.
///
/// Anything containing a `/` is a path.  A bare number, or `gpiochipN`, is a
/// chip number.  Everything else is a controller label — which is what
/// Rockchip and Allwinner boards need, since their bank names carry the
/// meaning that a Pi packs into a single header chip.
pub fn parse_selector(selector: &str) -> ChipSelector {
    let selector = selector.trim();

    if selector.contains('/') {
        return ChipSelector::Path(selector.to_string());
    }
    if let Ok(n) = selector.parse::<u32>() {
        return ChipSelector::Number(n);
    }
    if let Some(digits) = selector.strip_prefix("gpiochip") {
        if let Ok(n) = digits.parse::<u32>() {
            return ChipSelector::Number(n);
        }
    }
    ChipSelector::Label(selector.to_string())
}

/// `label (path, N lines)` for every controller the kernel exposes.
///
/// Used to tell a user on an unrecognised board exactly what they can put in
/// `gpio_chip`, rather than leaving them to guess.
pub fn describe_chips() -> Vec<String> {
    let Ok(chips) = gpio_cdev::chips() else {
        return Vec::new();
    };
    chips
        .flatten()
        .map(|c| {
            format!(
                "{} ({}, {} lines)",
                c.label(),
                c.path().display(),
                c.num_lines()
            )
        })
        .collect()
}

/// Open the controller named by `selector`.
pub fn open_chip(selector: &str) -> Result<Chip, GpioError> {
    match parse_selector(selector) {
        ChipSelector::Path(path) => open_path(&path),
        ChipSelector::Number(n) => open_path(&format!("/dev/gpiochip{}", n)),
        ChipSelector::Label(label) => open_label(&label),
    }
}

/// Open the GPIO controller carrying the display's DC/RST lines.
///
/// `preferred` is the configured `gpio_chip`, if any.  See the module header
/// for the full selection order.
pub fn open_header_chip(preferred: Option<&str>) -> Result<Chip, GpioError> {
    let configured = preferred
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var(CHIP_ENV_VAR).ok().filter(|s| !s.trim().is_empty()));

    if let Some(selector) = configured {
        let chip = open_chip(&selector)?;
        info!(
            "GPIO controller: {} ({}) [selected by '{}']",
            chip.label(),
            chip.path().display(),
            selector
        );
        return Ok(chip);
    }

    if let Ok(chips) = gpio_cdev::chips() {
        let mut found: Vec<Chip> = chips.flatten().collect();
        for label in HEADER_LABELS {
            if let Some(pos) = found.iter().position(|c| c.label() == label) {
                let chip = found.swap_remove(pos);
                info!("GPIO header controller: {} ({})", label, chip.path().display());
                return Ok(chip);
            }
        }
    }

    // Not a Pi — the fallback is a guess, so say so loudly and show the user
    // what they should have configured instead.
    warn!(
        "No Raspberry Pi header GPIO controller found; falling back to {}",
        DEFAULT_GPIO_CHIP
    );
    warn!(
        "On Orange Pi / Rockchip, Allwinner and other boards set display.bus.gpio_chip \
         (or {}) and give bank-relative dc_pin/rst_pin values",
        CHIP_ENV_VAR
    );
    for line in describe_chips() {
        warn!("  available GPIO controller: {}", line);
    }

    open_path(DEFAULT_GPIO_CHIP)
}

/// Request a GPIO line as an output with the given default level.
pub fn request_output(
    chip: &mut Chip,
    pin: u32,
    default: u8,
    consumer: &str,
) -> Result<CdevPin, GpioError> {
    if pin >= chip.num_lines() {
        return Err(GpioError(format!(
            "GPIO line {} is out of range for {} ({}), which has {} lines (0-{})",
            pin,
            chip.label(),
            chip.path().display(),
            chip.num_lines(),
            chip.num_lines().saturating_sub(1)
        )));
    }
    let line = chip
        .get_line(pin)
        .map_err(|e| GpioError(format!("GPIO line {}: {:?}", pin, e)))?;
    let handle = line
        .request(LineRequestFlags::OUTPUT, default, consumer)
        .map_err(|e| GpioError(format!("GPIO request {}: {:?}", pin, e)))?;
    CdevPin::new(handle).map_err(|e| GpioError(format!("GPIO pin {}: {:?}", pin, e)))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn open_path(path: &str) -> Result<Chip, GpioError> {
    Chip::new(path).map_err(|e| GpioError(format!("Failed to open {}: {:?}", path, e)))
}

fn open_label(label: &str) -> Result<Chip, GpioError> {
    if let Ok(chips) = gpio_cdev::chips() {
        let mut found: Vec<Chip> = chips.flatten().collect();
        if let Some(pos) = found.iter().position(|c| c.label() == label) {
            return Ok(found.swap_remove(pos));
        }
    }
    let available = describe_chips();
    let listing = if available.is_empty() {
        "none found — is gpiochip support enabled and are you in the 'gpio' group?".to_string()
    } else {
        available.join(", ")
    };
    Err(GpioError(format!(
        "No GPIO controller labelled '{}'; available: {}",
        label, listing
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Device nodes are taken literally.
    #[test]
    fn paths_are_recognised() {
        assert_eq!(
            parse_selector("/dev/gpiochip3"),
            ChipSelector::Path("/dev/gpiochip3".to_string())
        );
    }

    /// A bare number and the `gpiochipN` spelling mean the same chip.
    #[test]
    fn numbers_and_gpiochip_prefix_agree() {
        assert_eq!(parse_selector("4"), ChipSelector::Number(4));
        assert_eq!(parse_selector("gpiochip4"), ChipSelector::Number(4));
    }

    /// Rockchip banks and Pi/Allwinner pinctrl nodes are labels, not numbers —
    /// note that "gpio3" must NOT be read as chip 3, they are different things.
    #[test]
    fn controller_names_stay_labels() {
        assert_eq!(parse_selector("gpio3"), ChipSelector::Label("gpio3".to_string()));
        assert_eq!(
            parse_selector("pinctrl-rp1"),
            ChipSelector::Label("pinctrl-rp1".to_string())
        );
        assert_eq!(
            parse_selector("1c20800.pinctrl"),
            ChipSelector::Label("1c20800.pinctrl".to_string())
        );
    }

    /// Surrounding whitespace from a hand-edited YAML file is harmless.
    #[test]
    fn selectors_are_trimmed() {
        assert_eq!(parse_selector("  gpio3  "), ChipSelector::Label("gpio3".to_string()));
        assert_eq!(parse_selector(" 2 "), ChipSelector::Number(2));
    }
}
