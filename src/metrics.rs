/*
 *  metrics.rs
 * 
 *  LyMonS - worth the squeeze
 *	(c) 2020-25 Stuart Hunter
 *
 *	TODO:
 *
 *	This program is free software: you can redistribute it and/or modify
 *	it under the terms of the GNU General Public License as published by
 *	the Free Software Foundation, either version 3 of the License, or
 *	(at your option) any later version.
 *
 *	This program is distributed in the hope that it will be useful,
 *	but WITHOUT ANY WARRANTY; without even the implied warranty of
 *	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *	GNU General Public License for more details.
 *
 *	See <http://www.gnu.org/licenses/> to get a copy of the GNU General
 *	Public License.
 *
 */
//! A Rust library for gathering system metrics from /proc and /sys files.

use std::fs;
use std::io::{self, Read};

/// A struct to hold metrics information.
/// Corresponds to meminfo_t in the C code.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MachineMetrics {
    pub cpu_load: f64,
    pub cpu_temp: f64,
    pub up_time: f64,
    pub mem_total_kib: u64,
    pub mem_avail_mib: u64,
    pub mem_avail_pct: f64,
}

impl MachineMetrics {

    /// Reads the first float value from a given file path.
    /// This is a helper function to reduce code duplication.
    fn read_first_float_from_file(&mut self, path: &str) -> io::Result<f32> {
        let content = fs::read_to_string(path)?;
        let first_word = content.split_whitespace().next().unwrap_or("0.0");
        first_word.parse::<f32>().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Reads the first long value from a given file path.
    fn read_first_long_from_file(&mut self, path: &str) -> io::Result<i64> {
        let content = fs::read_to_string(path)?;
        let first_word = content.split_whitespace().next().unwrap_or("0");
        first_word.parse::<i64>().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Returns the 1-minute CPU load average as a percentage.
    /// Returns 0.0 on error.
    fn cpu_load(&mut self) -> f64 {
        match self.read_first_float_from_file("/proc/loadavg") {
            Ok(loadavg) => (100.0 * loadavg) as f64,
            Err(_) => 0.0,
        }
    }

    /// Returns the CPU temperature in Celsius.
    /// Reads from the first thermal zone. Returns 0.0 on error.
    fn cpu_temp(&mut self) -> f64 {
        match self.read_first_float_from_file("/sys/class/thermal/thermal_zone0/temp") {
            Ok(millideg) => {
                // The value is in millidegrees Celsius.
                (millideg / 1000.0) as f64
            }
            Err(_) => 0.0,
        }
    }

    /// Returns the system uptime in hours.
    /// Returns 0.0 on error.
    fn up_time(&mut self) -> f64 {
        match self.read_first_long_from_file("/proc/uptime") {
            Ok(uptime_seconds) => (uptime_seconds as f64) / 3600.0,
            Err(_) => 0.0,
        }
    }

    pub fn update(&mut self, metrics: MachineMetrics) {
        self.cpu_load = metrics.cpu_load;
        self.cpu_temp = metrics.cpu_temp;
        self.up_time = metrics.up_time;
    }

    pub fn check(&mut self) -> MachineMetrics {

        let mut metrics = MachineMetrics::default();

        metrics.cpu_load = self.cpu_load();
        metrics.cpu_temp = self.cpu_temp();
        metrics.up_time = self.up_time();

        /*
        {
            let mut file = fs::File::open("/proc/meminfo").unwrap();
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();    
            for line in contents.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();   
            }
        }
        */

        metrics
    }

}
