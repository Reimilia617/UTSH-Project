//! build.rs：编译 C++ FFI 侧代码并通过 `cc` crate 链接进 utsh-ffi。
//!
//! - 始终编译：`src/cpp/bash_parser.cpp`（当前最小解析器，无外部依赖）；
//! - `--features readline` 时额外编译 `readline_wrapper.cpp` 并链接 `-lreadline`；
//! - Bison/Flex 生成的 `grammar.tab.c` / `lexer.yy.c` 尚未接入（见 `grammar.y`
//!   头注释）；接入时在下方追加 `.file(...)` 即可。

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/cpp/bash_parser.cpp");
    println!("cargo:rerun-if-changed=src/cpp/bash_parser.h");
    println!("cargo:rerun-if-changed=src/cpp/readline_wrapper.cpp");

    let mut build = cc::Build::new();
    build
        .cpp(true) // 按 C++ 编译
        .std("c++17")
        .warnings(false)
        .include("src/cpp")
        .file("src/cpp/bash_parser.cpp");

    // CARGO_FEATURE_<name> 在构建脚本运行时由 Cargo 注入。
    if std::env::var("CARGO_FEATURE_READLINE").is_ok() {
        build.file("src/cpp/readline_wrapper.cpp");
        println!("cargo:rustc-link-lib=readline");
    }

    build.compile("utsh_ffi_cpp");
}
