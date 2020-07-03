use std::env;

pub fn main() {
    if let Ok(os) = env::var("CARGO_CFG_TARGET_OS") {
        if os == "redox" {
            println!("cargo:rustc-cfg=nightly");
        }
    }
}
