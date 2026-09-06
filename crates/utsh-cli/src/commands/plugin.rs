//! `utsh plugin` 子命令实现。

// CLI 表格输出中直接内联列头/列值字面量更直观，允许该 lint。
#![allow(clippy::print_literal)]

use std::path::Path;

use anyhow::{anyhow, Context};
use utsh_core::config::Config;
use utsh_core::plugins::{official, PluginManager, OFFICIAL_PLUGINS};

use crate::cli::PluginAction;

/// 内置插件名（与 §4.5 一致），随 `enable_defaults` 自动加载。
pub const BUILTIN_PLUGINS: &[&str] = &["zsh-syntax-highlighting", "zsh-autosuggestions"];

pub fn run(config_path: &Path, cfg: &mut Config, action: PluginAction) -> anyhow::Result<()> {
    match action {
        PluginAction::List => list(cfg),
        PluginAction::Browse { category } => browse(category.as_deref()),
        PluginAction::Install { name, source } => install(config_path, cfg, &name, &source),
        PluginAction::Uninstall { name } => uninstall(config_path, cfg, &name),
        PluginAction::Enable { name } => enable(config_path, cfg, &name),
        PluginAction::Disable { name } => disable(config_path, cfg, &name),
        PluginAction::Update { name } => update(name.as_deref()),
    }
}

fn list(cfg: &Config) -> anyhow::Result<()> {
    let manager = PluginManager::default_dir();
    let installed = manager.scan()?;

    println!(
        "installed plugins directory: {}",
        manager.plugins_dir.display()
    );
    println!();

    if cfg.plugins.enable_defaults {
        println!(
            "{:<30} {:<9} {:<10} {}",
            "NAME", "SOURCE", "ENABLED", "INSTALLED"
        );
        for builtin in BUILTIN_PLUGINS {
            let is_installed = installed.iter().any(|p| p.name == *builtin);
            println!(
                "{:<30} {:<9} {:<10} {}",
                builtin,
                "builtin",
                "yes",
                if is_installed {
                    "yes"
                } else {
                    "no (submodule)"
                }
            );
        }
        println!();
    }

    println!(
        "{:<30} {:<9} {:<10} {}",
        "NAME", "SOURCE", "ENABLED", "INSTALLED"
    );
    for info in installed {
        let enabled = if cfg.is_plugin_enabled(&info.name) {
            "yes"
        } else {
            "no"
        };
        println!("{:<30} {:<9} {:<10} {}", info.name, "git", enabled, "yes");
    }
    Ok(())
}

fn browse(category: Option<&str>) -> anyhow::Result<()> {
    println!(
        "{:<28} {:<11} {:<40} {}",
        "NAME", "CATEGORY", "REPO", "DESCRIPTION"
    );
    for p in OFFICIAL_PLUGINS {
        if let Some(cat) = category {
            if !cat.eq_ignore_ascii_case(p.category) {
                continue;
            }
        }
        println!(
            "{:<28} {:<11} {:<40} {}",
            p.name, p.category, p.repo, p.description
        );
    }
    println!();
    println!("install with: utsh plugin install <name>");
    Ok(())
}

fn install(config_path: &Path, cfg: &mut Config, name: &str, source: &str) -> anyhow::Result<()> {
    let manager = PluginManager::default_dir();
    // 来源缺省时按官方市场解析（官方名即 name），否则原样传 git 层。
    let effective_source = if source.is_empty() {
        match official(name) {
            Some(_) => name.to_string(), // 官方插件：名字本身可被 git 层解析
            None if name.contains('/') || name.contains("://") => name.to_string(),
            None => {
                return Err(anyhow!(
                    "unknown plugin `{name}`: pass a git URL or user/repo via --source"
                ))
            }
        }
    } else {
        source.to_string()
    };

    let manifest = manager
        .install(name, &effective_source, None)
        .with_context(|| format!("failed to install plugin `{name}`"))?;

    // 安装后自动启用（§4.6 表格注释）。
    if cfg.enable_plugin(name) {
        cfg.save_to(config_path)?;
    } else {
        eprintln!(
            "warning: plugin `{name}` installed but not enabled (max_plugins = {})",
            cfg.plugins.max_plugins
        );
    }

    println!("installed `{}` from {}", manifest.name, manifest.source);
    println!("run `utsh plugin list` to verify");
    Ok(())
}

fn uninstall(config_path: &Path, cfg: &mut Config, name: &str) -> anyhow::Result<()> {
    let manager = PluginManager::default_dir();
    manager
        .uninstall(name)
        .with_context(|| format!("failed to uninstall plugin `{name}`"))?;
    cfg.disable_plugin(name);
    cfg.save_to(config_path)?;
    println!("uninstalled `{name}`");
    Ok(())
}

fn enable(config_path: &Path, cfg: &mut Config, name: &str) -> anyhow::Result<()> {
    // 已安装校验（内置插件除外）。
    if !BUILTIN_PLUGINS.contains(&name) {
        let manager = PluginManager::default_dir();
        manager
            .get(name)
            .with_context(|| format!("plugin `{name}` is not installed"))?;
    }
    if cfg.enable_plugin(name) {
        cfg.save_to(config_path)?;
        println!("enabled `{name}`");
        Ok(())
    } else {
        Err(anyhow!(
            "cannot enable `{name}`: already enabled or max_plugins ({}) reached",
            cfg.plugins.max_plugins
        ))
    }
}

fn disable(config_path: &Path, cfg: &mut Config, name: &str) -> anyhow::Result<()> {
    if cfg.disable_plugin(name) {
        cfg.save_to(config_path)?;
        println!("disabled `{name}`");
        Ok(())
    } else {
        Err(anyhow!("plugin `{name}` is not enabled"))
    }
}

fn update(name: Option<&str>) -> anyhow::Result<()> {
    let manager = PluginManager::default_dir();
    let updated = manager.update(name).context("failed to update plugin(s)")?;
    if updated.is_empty() {
        println!("nothing to update");
    } else {
        println!("updated: {}", updated.join(", "));
    }
    Ok(())
}
