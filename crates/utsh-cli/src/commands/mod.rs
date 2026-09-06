//! 子命令实现分发。

pub mod alias;
pub mod doctor;
pub mod plugin;
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

/// WebUI 后端目录（相对本 crate 的工作区位置）。
pub(crate) fn webui_backend_dir() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = here.join("../../webui/backend");
    std::fs::canonicalize(&dir).unwrap_or(dir)
}

/// 顶层分发。
pub fn run(cli: Cli) -> anyhow::Result<()> {
    let Cli {
        command, config, ..
    } = cli;
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
    }
}
