//! 子命令实现分发。

pub mod alias;
pub mod doctor;
pub mod edit;
pub mod plugin;
pub mod shell;
pub mod status;
pub mod theme;
pub mod webui;

use std::path::PathBuf;

use anyhow::Context;
use utsh_core::config::Config;

use crate::cli::{Cli, Command};

/// 解析配置路径并加载配置（文件不存在时使用默认配置）。
pub(crate) fn resolve_config(override_path: &Option<PathBuf>) -> anyhow::Result<(PathBuf, Config)> {
    let path = override_path
        .clone()
        .unwrap_or_else(Config::default_config_path);
    let cfg = Config::load_from(&path)
        .with_context(|| format!("failed to load config: {}", path.display()))?;
    Ok((path, cfg))
}

/// WebUI 后端目录解析。
///
/// 候选顺序：`UTSH_WEBUI_DIR` → 开发态（crate 相对路径）→ 用户数据目录 →
/// `/usr/share/utsh` → `/usr/local/share/utsh`。首个含 `src/server.js` 的命中，
/// 保证源码树、deb/rpm 安装与手动安装三种场景都能工作。
pub(crate) fn webui_backend_dir() -> PathBuf {
    let dev_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../webui/backend");
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("UTSH_WEBUI_DIR") {
        candidates.push(PathBuf::from(dir));
    }
    candidates.push(dev_dir.clone());
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        candidates.push(PathBuf::from(xdg).join("utsh").join("webui/backend"));
    }
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join(".local/share/utsh/webui/backend"));
    }
    candidates.push(PathBuf::from("/usr/share/utsh/webui/backend"));
    candidates.push(PathBuf::from("/usr/local/share/utsh/webui/backend"));
    for c in candidates {
        if c.join("src/server.js").is_file() {
            return std::fs::canonicalize(&c).unwrap_or(c);
        }
    }
    // 兜底返回开发态路径（doctor / webui 会给出“后端缺失”提示）。
    dev_dir
}

/// 顶层分发。
pub fn run(cli: Cli) -> anyhow::Result<()> {
    let Cli {
        command, config, ..
    } = cli;

    // 无子命令 = 进入 Shell 会话（终端模拟器/登录 shell 均以裸 `utsh` 启动）。
    // TTY → 交互；非 TTY → 把 stdin 当脚本逐行执行。结束后带退出码结束。
    let Some(command) = command else {
        let code = shell::run(shell::ShellArgs {
            command: None,
            file: None,
            config,
        })?;
        std::process::exit(code);
    };

    match command {
        Command::Status => {
            let (path, cfg) = resolve_config(&config)?;
            status::run(&path, &cfg)
        }
        Command::Doctor => doctor::run(&config),
        Command::Alias(action) => {
            let (path, mut cfg) = resolve_config(&config)?;
            alias::run(&path, &mut cfg, action)
        }
        Command::Theme(action) => {
            let (path, mut cfg) = resolve_config(&config)?;
            theme::run(&path, &mut cfg, action)
        }
        Command::Plugin(action) => {
            let (path, mut cfg) = resolve_config(&config)?;
            plugin::run(&path, &mut cfg, action)
        }
        Command::Webui => webui::run(&config),
        Command::Shell { command, file } => {
            // 会话结束后以末条命令的退出码结束进程（`bash -c` 语义）。
            let code = shell::run(shell::ShellArgs {
                command,
                file,
                config,
            })?;
            std::process::exit(code);
        }
    }
}
