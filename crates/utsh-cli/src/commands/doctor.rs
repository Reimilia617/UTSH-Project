//! `utsh doctor` —— 环境诊断。

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;

use utsh_core::config::Config;

use super::{resolve_config, webui_backend_dir};

/// 一行检查结果。
struct Check {
    name: &'static str,
    passed: bool,
    detail: String,
}

fn report(c: &Check) {
    let mark = if c.passed { "PASS" } else { "FAIL" };
    println!("[{mark:>4}] {} — {}", c.name, c.detail);
}

fn command_ok(bin: &str) -> Option<String> {
    Command::new(bin)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or(bin)
                .trim()
                .to_string()
        })
}

pub fn run(override_config: &Option<PathBuf>) -> anyhow::Result<()> {
    println!("UTSH doctor ({})\n", crate::cli::VERSION);
    let mut checks: Vec<Check> = Vec::new();

    // 1. 配置解析
    let (cfg_path, cfg) = match resolve_config(override_config) {
        Ok(v) => v,
        Err(e) => {
            checks.push(Check {
                name: "config",
                passed: false,
                detail: e.to_string(),
            });
            report(&checks[0]);
            eprintln!("\ndoctor aborted: config cannot be loaded");
            return Err(anyhow::anyhow!(e));
        }
    };
    checks.push(Check {
        name: "config",
        passed: true,
        detail: format!("parsed OK ({})", cfg_path.display()),
    });

    // 2. 目录可创建且插件目录可写
    let dirs = [
        Config::config_dir(),
        Config::data_dir(),
        Config::plugins_dir(),
        Config::themes_dir(),
        Config::cache_dir(),
    ];
    let mut dir_ok = true;
    let mut dir_detail = String::new();
    for d in &dirs {
        if let Err(e) = std::fs::create_dir_all(d) {
            dir_ok = false;
            dir_detail = format!("cannot create {}: {e}", d.display());
            break;
        }
    }
    if dir_ok {
        // 写入探针验证可写
        let probe = Config::plugins_dir().join(".doctor-probe");
        let probe_ok =
            std::fs::write(&probe, b"ok").is_ok() && std::fs::remove_file(&probe).is_ok();
        if !probe_ok {
            dir_ok = false;
            dir_detail = format!("{} is not writable", Config::plugins_dir().display());
        } else {
            dir_detail = "all UT directories exist and are writable".into();
        }
    }
    checks.push(Check {
        name: "directories",
        passed: dir_ok,
        detail: dir_detail,
    });

    // 3. git（插件安装必需）
    match command_ok("git") {
        Some(v) => checks.push(Check {
            name: "git",
            passed: true,
            detail: v,
        }),
        None => checks.push(Check {
            name: "git",
            passed: false,
            detail: "not found — required for plugin install/update".into(),
        }),
    }

    // 4. node（webui 必需）
    match command_ok("node") {
        Some(v) => checks.push(Check {
            name: "node",
            passed: true,
            detail: v,
        }),
        None => checks.push(Check {
            name: "node",
            passed: false,
            detail: "not found — required for `utsh webui`".into(),
        }),
    }

    // 5. WebUI 后端目录
    let backend = webui_backend_dir();
    let backend_ok = backend.join("src/server.js").is_file();
    checks.push(Check {
        name: "webui-backend",
        passed: backend_ok,
        detail: if backend_ok {
            backend.display().to_string()
        } else {
            format!(
                "{} missing — run `pnpm --dir webui/backend install` first",
                backend.display()
            )
        },
    });

    // 6. 端口占用（仅提示，不计失败）
    if cfg.webui.enabled {
        match TcpListener::bind(("127.0.0.1", cfg.webui.port)) {
            Ok(_) => println!("[INFO] webui port 127.0.0.1:{} is free", cfg.webui.port),
            Err(_) => println!(
                "[INFO] webui port 127.0.0.1:{} is busy — another process may be listening",
                cfg.webui.port
            ),
        }
    }

    for c in &checks {
        report(c);
    }

    let failures = checks.iter().filter(|c| !c.passed).count();
    println!(
        "\n{}/{} checks passed{}",
        checks.len() - failures,
        checks.len(),
        if failures == 0 { " 🎉" } else { "" }
    );
    if failures > 0 {
        Err(anyhow::anyhow!("doctor: {failures} check(s) failed"))
    } else {
        Ok(())
    }
}
