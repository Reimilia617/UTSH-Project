//! # utsh-ffi —— Rust ↔ C++ FFI 胶水层（§4.1、§4.3）。
//!
//! 设计原则（文档 §8）：**C++ 部分尽量精简，只有在 Rust 无法直接调用系统库或
//! 现有 C 库时才使用 C++**。因此本 crate 只暴露极小、稳的 C ABI 面：
//!
//! | 函数 | 作用 |
//! | --- | --- |
//! | [`parse_bash_json`] | 解析 Bash 命令行 → AST JSON 字符串（安全封装） |
//! | `bindings::parse_bash` | C ABI：输入字符串 → `utsh_ast_node*` |
//! | `bindings::ast_to_json` | C ABI：AST → 堆上 JSON C 字符串 |
//!
//! 绑定说明：
//! - 绑定声明（`bindings` 模块）由 **bindgen** 从 `src/cpp/bash_parser.h` 生成，
//!   生成结果已提交到 `src/bindings.rs`，普通构建**不需要 libclang**；
//! - 需要重新生成时执行 `cargo run -p utsh-ffi --example gen_bindings`。
//!
//! Readline 封装在 `readline` feature 内（§4.3 的 C++ 终端层），默认关闭以免
//! 无头环境/CI 因缺少 libreadline 而构建失败。

#![warn(missing_docs)]

mod bindings;

use std::ffi::{CStr, CString};

/// 解析一段 Bash 命令行，返回 C++ 解析器生成的 AST JSON 字符串。
///
/// - 返回 `None`：输入为空 / 含内嵌 NUL / C++ 侧内部错误（如内存分配失败）；
/// - 返回 `Some(json)`：JSON 形态为
///   `{"kind":"command","text":"...","children":["echo","hi"]}`，
///   语义类型见 `utsh-core` 的 `parser::AstNode`。
pub fn parse_bash_json(input: &str) -> Option<String> {
    if input.trim().is_empty() {
        return None;
    }
    let c_input = CString::new(input).ok()?;
    // SAFETY: c_input 指向合法的 NUL 结尾字符串且不被其它线程修改；返回的
    // node/json 指针在使用后立即释放；拷贝在 free 之前完成。
    unsafe {
        let node = bindings::parse_bash(c_input.as_ptr());
        if node.is_null() {
            return None;
        }
        let json_ptr = bindings::ast_to_json(node);
        bindings::free_ast(node);
        if json_ptr.is_null() {
            return None;
        }
        let out = CStr::from_ptr(json_ptr).to_str().ok().map(str::to_owned);
        bindings::free_cstr(json_ptr);
        out
    }
}

#[cfg(feature = "readline")]
mod readline {
    //! GNU Readline 终端层封装（§4.3）。编译需系统安装 libreadline。
    //! 用法：`cargo build -p utsh-ffi --features readline`。

    use std::ffi::CString;
    use std::os::raw::{c_char, c_void};

    extern "C" {
        fn utsh_rl_readline(prompt: *const c_char) -> *mut c_char;
        fn utsh_rl_add_history(line: *const c_char);
        fn utsh_rl_clear_screen();
    }

    /// 读取一行（Readline）。返回 `None` 表示 EOF（Ctrl-D）。
    ///
    /// # Safety
    /// 内部释放由 libreadline 分配的返回指针，调用方无需管理。
    pub unsafe fn readline(prompt: &str) -> Option<String> {
        let c_prompt = CString::new(prompt).ok()?;
        // SAFETY: c_prompt 生命周期覆盖整个调用。
        let p = unsafe { utsh_rl_readline(c_prompt.as_ptr()) };
        if p.is_null() {
            return None;
        }
        // SAFETY: readline 返回 NUL 结尾字符串。
        let line = unsafe { std::ffi::CStr::from_ptr(p) }
            .to_string_lossy()
            .into_owned();
        // SAFETY: readline 内部用 malloc 分配，这里用 libc::free 释放。
        unsafe { libc::free(p.cast::<c_void>()) };
        Some(line)
    }

    /// 追加历史。
    pub fn add_history(line: &str) {
        if let Ok(c) = CString::new(line) {
            // SAFETY: c 生命周期覆盖调用。
            unsafe { utsh_rl_add_history(c.as_ptr()) };
        }
    }

    /// 清屏。
    pub fn clear_screen() {
        // SAFETY: 无参数无返回。
        unsafe { utsh_rl_clear_screen() };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_command() {
        let json = parse_bash_json("echo hello world").expect("parse ok");
        assert!(json.contains("\"kind\":\"command\""), "json = {json}");
        assert!(json.contains("\"echo\""), "json = {json}");
        assert!(json.contains("\"hello\""), "json = {json}");
        assert!(json.contains("\"world\""), "json = {json}");
    }

    #[test]
    fn quotes_are_handled() {
        let json = parse_bash_json("echo 'a b' \"c d\"").expect("parse ok");
        assert!(json.contains("\"a b\""), "json = {json}");
        assert!(json.contains("\"c d\""), "json = {json}");
    }

    #[test]
    fn empty_returns_none() {
        assert_eq!(parse_bash_json(""), None);
        assert_eq!(parse_bash_json("   \t "), None);
    }

    #[test]
    fn special_characters_are_json_escaped() {
        let json = parse_bash_json("echo \"a\\\"b\"").expect("parse ok");
        // 反斜杠与引号必须被 JSON 转义，保证 serde_json 能解析。
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert!(v["children"][1].as_str().is_some());
    }
}
