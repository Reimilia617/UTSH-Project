//! UTSH 命令行入口（§4.6）。
//!
//! 子命令树与设计文档一一对应：
//!
//! ```text
//! utsh
//! ├── plugin  list | browse [category] | install <name> | uninstall <name>
//! │           | enable <name> | disable <name> | update [name]
//! ├── theme   list | set <name> | preview <name>
//! ├── alias   add <name> <command> | remove <name> | list
//! ├── status
//! ├── doctor
//! └── webui
//! ```

pub mod cli;
pub mod commands;

pub use cli::Cli;

/// 库入口：解析子命令并分发。`main()` 里的错误处理见 `main.rs`。
pub fn run(cli: Cli) -> anyhow::Result<()> {
    commands::run(cli)
}
