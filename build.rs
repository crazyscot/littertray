#![allow(missing_docs)]

fn main() {
    // async closures weren't stabilised until 1.85
    if rustversion::cfg!(since(1.85)) {
        println!("cargo:rustc-cfg=async_supported");
    }
}
