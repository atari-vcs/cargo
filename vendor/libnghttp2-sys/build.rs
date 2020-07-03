extern crate pkg_config;

pub fn main() {
    if pkg_config::probe_library("libnghttp2").is_ok() {
        return;
    }
}
