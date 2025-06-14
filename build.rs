// build.rs

use chrono::Utc;
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Get the output directory set by Cargo
    let out_dir = env::var("OUT_DIR").unwrap();
    // Construct the path for the file where we'll write the build info
    let dest_path = Path::new(&out_dir).join("build_info.rs");

    // Get the current UTC time
    let now = Utc::now();
    // Format the date/time string as desired
    let build_date = now.format("%Y-%m-%d %H:%M:%S UTC").to_string();

    // Write a Rust constant definition to the file
    // This constant will be available in your main code
    fs::write(
        &dest_path,
        format!("pub const BUILD_DATE: &str = \"{}\";", build_date),
    ).unwrap();

    // Tell Cargo to re-run this build script only if build.rs itself changes
    // This ensures the build date is updated on new builds.
    println!("cargo:rerun-if-changed=build.rs");
}
