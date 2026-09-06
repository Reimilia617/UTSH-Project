//! Bash 语法解析入口（§4.1）。
//!
//! 解析本体是 `crates/utsh-ffi` 中的 C++ 解析器：当前阶段 `bash_parser.cpp`
//! 内置一个极简的“简单命令”tokenizer（支持单/双引号与反斜杠转义），并预留了
//! Bison 语法框架（`crates/utsh-ffi/src/cpp/grammar.y`）作为完整 Bash 语法的
//! 演进路径。C++ 侧把 AST 序列化为 JSON 字符串返回，Rust 侧在这里反序列化为
//! 类型安全的 [`AstNode`]。
//!
//! 后续阶段会在本模块之上做语义分析（命令/关键字识别、`shopt` 选项、条件展开
//! `{a..b}`、`$(...)` 等），并把 AST 交给执行引擎。

use serde::{Deserialize, Serialize};

/// 一个来自 C++ 解析器的 AST 节点（JSON 形态保持极简，便于 FFI 往返）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstNode {
    /// 节点种类：`command` / `empty` / `error` / 未来由 Bison 语法产生的种类。
    pub kind: String,
    /// 该节点对应的原始输入文本（去掉首尾空白）。
    #[serde(default)]
    pub text: String,
    /// 简单命令的词元（argv）；复杂语法接入后可能演进为嵌套结构。
    #[serde(default)]
    pub children: Vec<String>,
}

impl AstNode {
    /// 是否为简单命令节点。
    pub fn is_command(&self) -> bool {
        self.kind == "command"
    }

    /// 命令名（argv 首项）。
    pub fn program(&self) -> Option<&str> {
        self.children.first().map(String::as_str)
    }

    /// 命令参数（不含命令名）。
    pub fn args(&self) -> &[String] {
        &self.children[1.min(self.children.len())..]
    }

    /// 节点对应的原始文本。
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// 解析结果。无法解析/解析器不可用时返回 [`ParseOutcome::Unsupported`]，调用方
/// 可回落到纯 Rust 的内置命令处理（scaffold 阶段的行为）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseOutcome {
    /// 解析成功，得到 AST。
    Parsed(AstNode),
    /// 语法暂不支持或解析器不可用。
    Unsupported,
}

/// 解析一段命令行输入。空输入视为“无命令”。
pub fn parse(input: &str) -> ParseOutcome {
    if input.trim().is_empty() {
        return ParseOutcome::Unsupported;
    }
    match utsh_ffi::parse_bash_json(input) {
        None => ParseOutcome::Unsupported,
        Some(json) => match serde_json::from_str::<AstNode>(&json) {
            Ok(node) if node.kind == "error" => ParseOutcome::Unsupported,
            Ok(node) => ParseOutcome::Parsed(node),
            Err(e) => {
                tracing::warn!(error = %e, json = %json, "parser returned malformed JSON");
                ParseOutcome::Unsupported
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_command() {
        // 简单命令由 C++ 侧最小解析器处理，因此这里不依赖完整 Bash 语法。
        let out = parse("echo hello world");
        match out {
            ParseOutcome::Parsed(node) => {
                assert!(node.is_command());
                assert_eq!(node.program(), Some("echo"));
                assert_eq!(node.args(), &["hello", "world"]);
            }
            ParseOutcome::Unsupported => panic!("simple command should parse"),
        }
    }

    #[test]
    fn empty_input_is_unsupported() {
        assert_eq!(parse(""), ParseOutcome::Unsupported);
        assert_eq!(parse("   "), ParseOutcome::Unsupported);
    }
}
