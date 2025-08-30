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
