extern crate napi_build;

fn main() {
    napi_build::setup();

    // Capture rustc version at build time (used by get_rust_version)
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let output = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .expect("Failed to get rustc version");

    let version = String::from_utf8_lossy(&output.stdout);
    let version = version.trim();

    println!("cargo:rustc-env=RUSTC_VERSION={}", version);
    println!("cargo:rerun-if-changed=build.rs");
}
