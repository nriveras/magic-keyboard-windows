//! Build script for the Windows Rust Driver crate.
//!
//! Based on the [`wdk_build::Config`] parsed from the build tree, this build
//! script will provide `Cargo` with the necessary information to build the
//! driver binary (ex. linker flags)

fn main() -> Result<(), wdk_build::ConfigError> {
    // The wdk-sys "hid" feature generates HidP_* bindings but only links VhfKm; the parser
    // routines live in hidparse.sys (import library in the WDK km lib directory).
    println!("cargo::rustc-link-lib=hidparse");
    wdk_build::configure_wdk_binary_build()
}
