//! `utsh theme` 子命令实现。
//!
//! 注意（§8 补充说明）：**默认不安装任何主题**——`current = "none"`。
//! 用户必须通过 `utsh theme set` 或 WebUI 主动安装/选择主题。

use std::path::Path;

use anyhow::{anyhow, Context};
use utsh_core::config::Config;

use crate::cli::ThemeAction;

/// 主题 CSS 约定名：`<themes_dir>/<name>/<name>.css`。
fn theme_css_path(name: &str) -> std::path::PathBuf {
    Config::themes_dir().join(name).join(format!("{name}.css"))
}

/// 列出可用主题（含内置 `none`）。
fn list_themes() -> Vec<String> {
    let mut out = vec!["none".to_string()];
    let dir = Config::themes_dir();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                out.push(e.file_name().to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    out
}

pub fn run(config_path: &Path, cfg: &mut Config, action: ThemeAction) -> anyhow::Result<()> {
    match action {
        ThemeAction::List => {
            let current = cfg.theme();
            println!("themes directory: {}", Config::themes_dir().display());
            println!();
            for t in list_themes() {
                if t == current {
                    println!("* {t}   (current)");
                } else {
                    println!("  {t}");
                }
            }
            println!();
            println!("apply with: utsh theme set <name>");
            Ok(())
        }
        ThemeAction::Set { name } => {
            if !list_themes().iter().any(|t| t == &name) {
                return Err(anyhow!(
                    "theme `{name}` is not installed. Use `utsh theme list`; \
                     install themes from the WebUI (`utsh webui`)."
                ));
            }
            cfg.set_theme(name.as_str());
            cfg.save_to(config_path)?;
            println!("theme set to `{name}`");
            Ok(())
        }
        ThemeAction::Preview { name } => {
            let css = theme_css_path(&name);
            if !css.is_file() {
                return Err(anyhow!(
                    "theme `{name}` has no preview CSS at {} — theme preview is \
                     rendered in the WebUI (`utsh webui`)",
                    css.display()
                ));
            }
            let content = std::fs::read_to_string(&css)
                .with_context(|| format!("failed to read {}", css.display()))?;
            println!("== {name} ({} lines) ==", content.lines().count());
            for line in content.lines().take(40) {
                println!("{line}");
            }
            Ok(())
        }
    }
}
