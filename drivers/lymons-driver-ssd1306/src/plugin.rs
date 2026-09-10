/*
 *  LyMonS SSD1306 Plugin - Driver Implementation
 *
 *  Implements the SSD1306 OLED display driver as a LyMonS plugin
 */

use std::ffi::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};
use ssd1306::{
    mode::BufferedGraphicsMode,
    prelude::*,
    I2CDisplayInterface,
    Ssd1306,
};
use linux_embedded_hal::{I2cdev, SpidevDevice, CdevPin};
use linux_embedded_hal::spidev::{SpidevOptions, SpiModeFlags};
use lymons_gpio as gpio;
use embedded_hal::digital::OutputPin;

use crate::ffi::*;

/// Default SPI clock when the host supplies 0
const DEFAULT_SPI_SPEED_HZ: u32 = 8_000_000;

/// The SSD1306 controller supports both I2C and SPI; the concrete interface
/// type differs, so the live display is held in a small enum.
enum Ssd1306Display {
    I2c(Ssd1306<I2CInterface<I2cdev>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>),
    Spi(Ssd1306<SPIInterface<SpidevDevice, CdevPin>, DisplaySize128x64, BufferedGraphicsMode<DisplaySize128x64>>),
}

/// Run an expression against the active display regardless of bus type.
/// The body is type-checked per arm, so identical source serves both interfaces.
macro_rules! disp {
    ($self:ident, $d:ident => $body:expr) => {
        match &mut $self.display {
            Ssd1306Display::I2c($d) => $body,
            Ssd1306Display::Spi($d) => $body,
        }
    };
}

/// Internal SSD1306 driver state
pub struct Ssd1306PluginDriver {
    /// The actual SSD1306 driver from the ssd1306 crate, over the selected bus
    display: Ssd1306Display,

    /// Display capabilities
    capabilities: LyMonsDisplayCapabilities,

    /// Current brightness (0-255)
    brightness: u8,

    /// Current inversion state
    inverted: bool,

    /// Reset line held high for the driver's lifetime (SPI only)
    _rst: Option<CdevPin>,
}

impl Ssd1306PluginDriver {
    /// Create a new SSD1306 driver from configuration (I2C or SPI)
    pub fn new(config: &LyMonsDisplayConfig) -> Result<Self, String> {
        // Open whichever bus the host selected and build the display
        let (display, rst) = match config.bus.bus_type {
            LyMonsBusType::I2c => {
                let i2c_config = unsafe { &config.bus.config.i2c };
                let bus_path = extract_string_from_buffer(&i2c_config.bus_path);

                let i2c = I2cdev::new(&bus_path)
                    .map_err(|e| format!("Failed to open I2C device {}: {:?}", bus_path, e))?;
                let interface = I2CDisplayInterface::new(i2c);
                let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
                    .into_buffered_graphics_mode();
                display.init()
                    .map_err(|e| format!("Failed to initialize display: {:?}", e))?;
                (Ssd1306Display::I2c(display), None)
            }
            LyMonsBusType::Spi => {
                let spi_config = unsafe { &config.bus.config.spi };
                let bus_path = extract_string_from_buffer(&spi_config.bus_path);
                Self::open_spi(&bus_path, spi_config.dc_pin, spi_config.rst_pin, spi_config.speed_hz)?
            }
        };

        let brightness = if config.has_brightness { config.brightness } else { 128 };

        let capabilities = LyMonsDisplayCapabilities {
            width: 128,
            height: 64,
            color_depth: LyMonsColorDepth::Monochrome,
            supports_rotation: true,
            max_fps: 60,
            supports_brightness: true,
            supports_invert: true,
        };

        let mut driver = Self {
            display,
            capabilities,
            brightness,
            inverted: config.inverted,
            _rst: rst,
        };

        // Apply initial configuration through the shared, bus-agnostic methods
        driver.set_brightness(brightness)?;
        if config.has_rotation {
            driver.set_rotation(config.rotation)?;
        }
        if config.inverted {
            driver.set_invert(true)?;
        }

        Ok(driver)
    }

    /// Open and configure the SPI bus + DC/RST GPIO lines, then build the display.
    /// A `rst_pin` of 0 means no hardware reset line is wired.
    fn open_spi(
        bus_path: &str,
        dc_pin: u8,
        rst_pin: u8,
        speed_hz: u32,
    ) -> Result<(Ssd1306Display, Option<CdevPin>), String> {
        let speed = if speed_hz == 0 { DEFAULT_SPI_SPEED_HZ } else { speed_hz };

        let mut spi = SpidevDevice::open(bus_path)
            .map_err(|e| format!("Failed to open SPI {}: {:?}", bus_path, e))?;
        let options = SpidevOptions::new()
            .bits_per_word(8)
            .max_speed_hz(speed)
            .mode(SpiModeFlags::SPI_MODE_0)
            .build();
        spi.0.configure(&options)
            .map_err(|e| format!("Failed to configure SPI: {:?}", e))?;

        // Plugins do not see the application's YAML, so a non-Pi board selects
        // its GPIO controller through the LYMONS_GPIO_CHIP environment variable.
        let mut chip = gpio::open_header_chip(None).map_err(|e| e.into_message())?;
        let dc = gpio::request_output(&mut chip, dc_pin as u32, 0, "lymons-ssd1306-dc")
            .map_err(|e| e.into_message())?;

        // Hardware reset pulse, held high afterwards for the driver's lifetime
        let rst = if rst_pin != 0 {
            let mut rst = gpio::request_output(&mut chip, rst_pin as u32, 1, "lymons-ssd1306-rst")
                .map_err(|e| e.into_message())?;
            rst.set_high().ok();
            std::thread::sleep(std::time::Duration::from_millis(1));
            rst.set_low().ok();
            std::thread::sleep(std::time::Duration::from_millis(10));
            rst.set_high().ok();
            std::thread::sleep(std::time::Duration::from_millis(10));
            Some(rst)
        } else {
            None
        };

        let interface = SPIInterface::new(spi, dc);
        let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        display.init()
            .map_err(|e| format!("Failed to initialize display: {:?}", e))?;

        Ok((Ssd1306Display::Spi(display), rst))
    }


    /// Initialize the display (called after creation)
    pub fn init(&mut self) -> Result<(), String> {
        disp!(self, d => {
            d.clear_buffer();
            d.flush().map_err(|e| format!("Failed to flush display: {:?}", e))
        })
    }

    /// Set display brightness (0-255)
    pub fn set_brightness(&mut self, value: u8) -> Result<(), String> {
        self.brightness = value;

        let brightness_level = match value {
            0..=63 => Brightness::DIMMEST,
            64..=127 => Brightness::DIM,
            128..=191 => Brightness::NORMAL,
            192..=255 => Brightness::BRIGHTEST,
        };

        disp!(self, d =>
            d.set_brightness(brightness_level)
                .map_err(|e| format!("Failed to set brightness: {:?}", e))
        )
    }

    /// Flush the framebuffer to the display
    pub fn flush(&mut self) -> Result<(), String> {
        disp!(self, d => d.flush().map_err(|e| format!("Failed to flush: {:?}", e)))
    }

    /// Clear the display
    pub fn clear(&mut self) -> Result<(), String> {
        disp!(self, d => {
            d.clear_buffer();
            d.flush().map_err(|e| format!("Failed to clear: {:?}", e))
        })
    }

    /// Write raw buffer to display
    pub fn write_buffer(&mut self, buffer: &[u8]) -> Result<(), String> {
        // SSD1306 uses 128x64 = 8192 pixels = 1024 bytes
        let expected_size = 1024;

        if buffer.len() != expected_size {
            return Err(format!(
                "Buffer size mismatch: expected {} bytes, got {}",
                expected_size,
                buffer.len()
            ));
        }

        // Copy buffer to display buffer. The ssd1306 crate doesn't expose direct
        // buffer access, so we clear and redraw pixel-by-pixel. Resolve the active
        // display once so the enum match isn't repeated per pixel.
        disp!(self, d => {
            d.clear_buffer();
            for (byte_idx, &byte) in buffer.iter().enumerate() {
                let page = byte_idx / 128; // 8 pages (8 pixels high each)
                let col = byte_idx % 128;
                for bit in 0..8 {
                    let y = (page * 8 + bit) as u32;
                    let x = col as u32;
                    if (byte >> bit) & 1 == 1 {
                        let _ = d.set_pixel(x, y, true);
                    }
                }
            }
            d.flush().map_err(|e| format!("Failed to write buffer: {:?}", e))
        })
    }

    /// Set display inversion
    pub fn set_invert(&mut self, inverted: bool) -> Result<(), String> {
        self.inverted = inverted;

        // SSD1306 supports inversion via command
        disp!(self, d =>
            d.set_display_on(!inverted)
                .map_err(|e| format!("Failed to set invert: {:?}", e))
        )
    }

    /// Set display rotation
    pub fn set_rotation(&mut self, degrees: u16) -> Result<(), String> {
        let rotation = match degrees {
            0 => DisplayRotation::Rotate0,
            90 => DisplayRotation::Rotate90,
            180 => DisplayRotation::Rotate180,
            270 => DisplayRotation::Rotate270,
            _ => return Err(format!("Invalid rotation: {}", degrees)),
        };

        disp!(self, d =>
            d.set_rotation(rotation)
                .map_err(|e| format!("Failed to set rotation: {:?}", e))
        )
    }

    /// Get display capabilities
    pub fn capabilities(&self) -> &LyMonsDisplayCapabilities {
        &self.capabilities
    }
}

/// Macro to catch panics in FFI functions
macro_rules! catch_panic {
    ($error:expr, $code:block) => {
        match catch_unwind(AssertUnwindSafe(|| $code)) {
            Ok(result) => result,
            Err(panic_info) => {
                let message = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    format!("Plugin panic: {}", s)
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    format!("Plugin panic: {}", s)
                } else {
                    "Plugin panic: unknown error".to_string()
                };

                unsafe {
                    *$error = LyMonsError::new(LyMonsErrorCode::ErrorPanic, &message);
                }
                LyMonsErrorCode::ErrorPanic
            }
        }
    };
}

// ============================================================================
// FFI Vtable Implementations
// ============================================================================

/// Get plugin ABI version
extern "C" fn abi_version(major: *mut u32, minor: *mut u32, patch: *mut u32) {
    if !major.is_null() && !minor.is_null() && !patch.is_null() {
        unsafe {
            *major = LYMONS_PLUGIN_ABI_VERSION_MAJOR;
            *minor = LYMONS_PLUGIN_ABI_VERSION_MINOR;
            *patch = LYMONS_PLUGIN_ABI_VERSION_PATCH;
        }
    }
}

/// Get plugin metadata
extern "C" fn plugin_info(
    name: *mut c_char,
    version: *mut c_char,
    driver_type: *mut c_char
) {
    copy_str_to_buffer("LyMonS SSD1306 Driver", name, 64);
    copy_str_to_buffer("1.0.0", version, 32);
    copy_str_to_buffer("ssd1306", driver_type, 32);
}

/// Create a new driver instance
extern "C" fn create(
    config: *const LyMonsDisplayConfig,
    handle: *mut *mut LyMonsDriverHandle,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        // Validate pointers
        if config.is_null() || handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to create"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        // Create driver
        let driver = match Ssd1306PluginDriver::new(unsafe { &*config }) {
            Ok(d) => d,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorInitialization, &e);
                }
                return LyMonsErrorCode::ErrorInitialization;
            }
        };

        // Convert to opaque handle
        unsafe {
            *handle = Box::into_raw(Box::new(driver)) as *mut LyMonsDriverHandle;
        }

        LyMonsErrorCode::Success
    })
}

/// Destroy a driver instance
extern "C" fn destroy(handle: *mut LyMonsDriverHandle) {
    if !handle.is_null() {
        unsafe {
            let _ = Box::from_raw(handle as *mut Ssd1306PluginDriver);
        }
    }
}

/// Get driver capabilities
extern "C" fn capabilities(
    handle: *const LyMonsDriverHandle,
    caps: *mut LyMonsDisplayCapabilities,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || caps.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to capabilities"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &*(handle as *const Ssd1306PluginDriver) };
        unsafe {
            *caps = *driver.capabilities();
        }

        LyMonsErrorCode::Success
    })
}

/// Initialize the display
extern "C" fn init(
    handle: *mut LyMonsDriverHandle,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to init"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.init() {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorInitialization, &e);
                }
                LyMonsErrorCode::ErrorInitialization
            }
        }
    })
}

/// Set display brightness
extern "C" fn set_brightness(
    handle: *mut LyMonsDriverHandle,
    value: u8,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to set_brightness"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.set_brightness(value) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e);
                }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

/// Flush framebuffer to display
extern "C" fn flush(
    handle: *mut LyMonsDriverHandle,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to flush"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.flush() {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e);
                }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

/// Clear the display
extern "C" fn clear(
    handle: *mut LyMonsDriverHandle,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to clear"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.clear() {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e);
                }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

/// Write raw buffer to display
extern "C" fn write_buffer(
    handle: *mut LyMonsDriverHandle,
    buffer: *const u8,
    length: usize,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || buffer.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to write_buffer"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };
        let buffer_slice = unsafe { std::slice::from_raw_parts(buffer, length) };

        match driver.write_buffer(buffer_slice) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorInvalidArgument, &e);
                }
                LyMonsErrorCode::ErrorInvalidArgument
            }
        }
    })
}

/// Set display inversion
extern "C" fn set_invert(
    handle: *mut LyMonsDriverHandle,
    inverted: bool,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to set_invert"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.set_invert(inverted) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorCommunication, &e);
                }
                LyMonsErrorCode::ErrorCommunication
            }
        }
    })
}

/// Set display rotation
extern "C" fn set_rotation(
    handle: *mut LyMonsDriverHandle,
    degrees: u16,
    error: *mut LyMonsError
) -> LyMonsErrorCode {
    catch_panic!(error, {
        if handle.is_null() || error.is_null() {
            unsafe {
                *error = LyMonsError::new(
                    LyMonsErrorCode::ErrorNullPointer,
                    "Null pointer passed to set_rotation"
                );
            }
            return LyMonsErrorCode::ErrorNullPointer;
        }

        let driver = unsafe { &mut *(handle as *mut Ssd1306PluginDriver) };

        match driver.set_rotation(degrees) {
            Ok(_) => LyMonsErrorCode::Success,
            Err(e) => {
                unsafe {
                    *error = LyMonsError::new(LyMonsErrorCode::ErrorInvalidRotation, &e);
                }
                LyMonsErrorCode::ErrorInvalidRotation
            }
        }
    })
}

// ============================================================================
// Plugin Registration
// ============================================================================

/// Static vtable
static VTABLE: LyMonsPluginVTable = LyMonsPluginVTable {
    abi_version,
    plugin_info,
    create,
    destroy,
    capabilities,
    init,
    set_brightness,
    flush,
    clear,
    write_buffer,
    set_invert,
    set_rotation,
};

/// Plugin entry point - returns the vtable
#[no_mangle]
pub extern "C" fn lymons_plugin_register() -> *const LyMonsPluginVTable {
    &VTABLE
}
