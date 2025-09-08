/*
 *  func_timers.rs
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
use std::time::Instant;
use log::debug;

// need to tie this log level
// if debug - setup
pub struct FunctionTimer {
    name: &'static str,
    start: Instant,
}

impl FunctionTimer {
     pub fn new(name: &'static str) -> Self {
        FunctionTimer {
            name,
            start: Instant::now(),
        }
    }
}

// This `Drop` implementation is called automatically when the `FunctionTimer` struct goes out of scope.
impl Drop for FunctionTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed();
        debug!("Function '{}' took: {:?}", self.name, duration);
    }
}