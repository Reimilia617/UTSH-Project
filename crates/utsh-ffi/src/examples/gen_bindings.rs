//! Regenerate `src/bindings.rs` from `src/cpp/bash_parser.h` via bindgen.
//!
//! Usage:
//! ```text
//! cargo run -p utsh-ffi --example gen_bindings
//! ```
//!
//! Requires libclang (e.g. `libclang-dev` on Debian/Ubuntu or the `clang`
//! Homebrew formula on macOS). Plain `cargo build` never needs this tool —
//! `src/bindings.rs` is committed to the repository.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let header = "src/cpp/bash_parser.h";
    println!("Generating bindings from {header} ...");
    let bindings = bindgen::Builder::default()
        .header(header)
        .allowlist_function("parse_bash")
        .allowlist_function("ast_to_json")
        .allowlist_function("free_ast")
        .allowlist_function("free_cstr")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()?;
    bindings.write_to_file("src/bindings.rs")?;
    println!("wrote src/bindings.rs");
    Ok(())
}
