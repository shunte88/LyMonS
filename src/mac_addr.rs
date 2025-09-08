/*
 *  mac_addrs.rs
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
// mac_addrs.rs (linux-only, no extra deps)
use std::fs;
use std::io;

pub fn get_mac_addr_for(ifname: &str) -> io::Result<String> {
    let p = format!("/sys/class/net/{}/address", ifname);
    let s = fs::read_to_string(p)?;
    Ok(s.trim().to_ascii_lowercase())
}

// keep your existing fallback if you like:
pub fn get_mac_addr() -> String {
    use mac_address::get_mac_address;
    get_mac_address().unwrap().unwrap().to_string().to_ascii_lowercase()
}
