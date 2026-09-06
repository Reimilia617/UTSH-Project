//! `utsh alias` 子命令实现。

// CLI 表格输出直接内联列头字面量更直观，允许该 lint。
#![allow(clippy::print_literal)]

use std::path::Path;

use anyhow::Context;
use utsh_core::config::Config;

use crate::cli::AliasAction;

pub fn run(config_path: &Path, cfg: &mut Config, action: AliasAction) -> anyhow::Result<()> {
    match action {
        AliasAction::Add { name, command } => {
            cfg.add_alias(&name, &command)
                .with_context(|| format!("failed to add alias `{name}`"))?;
            cfg.save_to(config_path)?;
            println!("alias `{name}` = `{command}`");
            Ok(())
        }
        AliasAction::Remove { name } => {
            if cfg.remove_alias(&name) {
                cfg.save_to(config_path)?;
                println!("removed alias `{name}`");
                Ok(())
            } else {
                Err(anyhow::anyhow!("alias `{name}` not found"))
            }
        }
        AliasAction::List => {
            let aliases = cfg.aliases();
            println!("{:<20} {}", "NAME", "COMMAND");
            for (name, cmd) in aliases {
                println!("{:<20} {}", name, cmd);
            }
            println!();
            println!("{} alias(es) in {}", aliases.len(), config_path.display());
            Ok(())
        }
    }
}
