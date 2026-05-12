use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=web/dist");
    println!("cargo:rerun-if-env-changed=ALCHEMIST_VERSION");
    println!("cargo:rerun-if-env-changed=ALCHEMIST_UPDATE_PUBLIC_KEY_B64");

    if let Some(version) = env::var_os("ALCHEMIST_VERSION") {
        println!(
            "cargo:rustc-env=ALCHEMIST_BUILD_VERSION={}",
            version.to_string_lossy()
        );
    }

    if let Some(public_key) = env::var_os("ALCHEMIST_UPDATE_PUBLIC_KEY_B64") {
        println!(
            "cargo:rustc-env=ALCHEMIST_UPDATE_PUBLIC_KEY_B64={}",
            public_key.to_string_lossy()
        );
    }

    if env::var_os("CARGO_FEATURE_EMBED_WEB").is_none() {
        return;
    }

    let dist_dir = Path::new("web/dist");
    if let Err(err) = fs::create_dir_all(dist_dir) {
        panic!("failed to create web/dist for embed-web feature: {err}");
    }
}
