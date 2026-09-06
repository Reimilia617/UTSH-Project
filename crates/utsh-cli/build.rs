//! build.rs：把根目录 `VERSION`（用户可见版本，如 26v1 / 26v1.1）注入为
//! `UTSH_RELEASE_VERSION`，供 `utsh --version` / `status` / `doctor` 显示。
//! 缺失时回落到 Cargo 包版本。

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let version_file = manifest.join("../../VERSION");
    let mut version = env!("CARGO_PKG_VERSION").to_string();
    if let Ok(raw) = fs::read_to_string(&version_file) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            version = trimmed.to_string();
            println!("cargo:rerun-if-changed={}", version_file.display());
        }
    }
    println!("cargo:rustc-env=UTSH_RELEASE_VERSION={version}");
}
