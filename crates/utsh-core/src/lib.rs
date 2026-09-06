//! # utsh-core
//!
//! UTSH（UT Shell）核心库。UTSH 是一个兼容 Bash 语法、同时具备 Zsh 插件与主题
//! 生态兼容能力的交互式 Shell，是 UT 系列生态（utnixos、utniri 等）的核心组件。
//!
//! 本 crate 只包含**纯逻辑核心**，不含终端交互主循环（由上层二进制 / Shell 会话
//! 驱动），以便单元测试与将来接入不同前端（CLI REPL、WebUI、测试驱动）时复用。
//!
//! ## 模块总览
//!
//! | 模块 | 说明 | 对应设计文档 |
//! | --- | --- | --- |
//! | [`config`] | `~/.config/ut/utsh.toml` 配置解析/写入 | §4.8 |
//! | [`parser`] | Bash 语法解析入口（经 `utsh-ffi` 调用 C++ Bison 解析器） | §4.1 |
//! | [`zle`] | ZLE 行编辑器模拟（状态机 / widget / bindkey / undo） | §4.3 |
//! | [`hooks`] | `preexec` / `precmd` 钩子系统 | §4.2 |
//! | [`job_control`] | 作业控制（进程组、前后台、信号） | §4.1 |
//! | [`plugins`] | 插件加载器与 git 安装 | §4.4 |
//! | [`alias`] | 别名表与命令行展开 | §4.6 |
//! | [`history`] | 历史记录管理 | §4.3 |
//! | [`ipc`] | Unix Domain Socket JSON-RPC 2.0 服务（供 Node.js WebUI 调用） | §4.9 |
//!
//! ## 设计约束
//!
//! * 启动性能：空配置加载 < 50ms，因此配置模块不做任何网络/磁盘之外的额外工作，
//!   所有插件按需懒加载。
//! * C++ 仅在 Rust 无法直接调用系统/现有 C 库时引入（见 `utsh-ffi`）。

#![deny(unsafe_op_in_unsafe_fn)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod alias;
pub mod config;
pub mod history;
pub mod hooks;
/// Unix Domain Socket JSON-RPC 服务，仅在 Unix 目标上可用。
#[cfg(unix)]
pub mod ipc;
pub mod job_control;
pub mod parser;
pub mod plugins;
pub mod zle;

pub use config::{Config, ConfigError};

/// 程序名常量。
pub const NAME: &str = "utsh";
/// crate 版本（与 Cargo.toml 同步）。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 常用 Result 别名。
pub type Result<T, E = ConfigError> = std::result::Result<T, E>;
