//! `utsh status` —— 显示当前状态。

use std::path::Path;

use utsh_core::config::Config;
use utsh_core::plugins::PluginManager;

use super::plugin::BUILTIN_PLUGINS;

pub fn run(config_path: &Path, cfg: &Config) -> anyhow::Result<()> {
    let installed = PluginManager::default_dir().scan().unwrap_or_default();
    let builtin_enabled = if cfg.plugins.enable_defaults {
        BUILTIN_PLUGINS.len()
    } else {
        0
    };
    let enabled_third_party = cfg.enabled_plugins().len();
    let config_exists = config_path.is_file();

    println!("UTSH status (utsh v{})", utsh_core::VERSION);
    println!("  parser backend : C++ FFI (utsh-ffi / bash_parser)");
    println!(
        "  config path    : {} {}",
        config_path.display(),
        if config_exists {
            "(exists)"
        } else {
            "(missing — using defaults)"
        }
    );
    println!("  plugins dir    : {}", Config::plugins_dir().display());
    println!(
        "  plugins        : {} builtin enabled + {} third-party enabled, {} installed",
        builtin_enabled,
        enabled_third_party,
        installed.len()
    );
    if !cfg.enabled_plugins().is_empty() {
        println!("    enabled      : {}", cfg.enabled_plugins().join(", "));
    }
    if cfg.plugins.enable_defaults {
        println!(
            "    defaults     : {} (enable_defaults = true)",
            BUILTIN_PLUGINS.join(", ")
        );
    }
    println!("  theme          : {} (default: none)", cfg.theme());
    println!("  aliases        : {}", cfg.aliases().len());
    println!("  default editor : {}", cfg.general.default_editor);
    println!(
        "  webui          : http://127.0.0.1:{} {}",
        cfg.webui.port,
        if cfg.webui.enabled {
            "(enabled)"
        } else {
            "(disabled)"
        }
    );
    if cfg.webui.auth_token.is_some() {
        println!("  webui token    : configured (send X-UTSH-Token header)");
    } else {
        println!("  webui token    : not configured — generated on first `utsh webui` start");
    }
    Ok(())
}
