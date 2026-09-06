//! `utsh webui` —— 启动 WebUI 后端并打开浏览器（§4.6、§5 WebUI）。

use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};

use super::resolve_config;

/// 轮询等待端口可连接。
fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// 用系统默认浏览器打开 URL（best-effort）。
fn open_browser(url: &str) {
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = Command::new("xdg-open");
        c.arg(url);
        c
    };
    match cmd.spawn() {
        Ok(_) => {}
        Err(e) => println!("note: could not auto-open browser ({e}) — open {url} manually"),
    }
}

pub fn run(override_config: &Option<PathBuf>) -> anyhow::Result<()> {
    let (cfg_path, cfg) = resolve_config(override_config)?;

    if !cfg.webui.enabled {
        return Err(anyhow!(
            "webui is disabled in {} ([webui] enabled = false)",
            cfg_path.display()
        ));
    }

    // 1. 后端目录就绪？
    let backend = super::webui_backend_dir();
    let entry = backend.join("src/server.js");
    if !entry.is_file() {
        return Err(anyhow!(
            "WebUI backend not found at {} — run `pnpm --dir webui/backend install` first",
            backend.display()
        ));
    }

    // 2. node 存在？
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !node_ok {
        return Err(anyhow!(
            "node.js not found — required to run the WebUI backend"
        ));
    }

    // 3. 启动后端（继承日志输出，便于调试）。
    let mut child = Command::new("node")
        .arg("src/server.js")
        .current_dir(&backend)
        .env("UTSH_CONFIG", &cfg_path)
        .env("UTSH_PORT", cfg.webui.port.to_string())
        .env("UTSH_HOST", "127.0.0.1")
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn WebUI backend")?;

    // 4. 等待就绪。
    let url = format!("http://127.0.0.1:{}", cfg.webui.port);
    if !wait_for_port(cfg.webui.port, Duration::from_secs(8)) {
        let _ = child.kill();
        return Err(anyhow!(
            "WebUI backend did not become ready on {url} within 8s"
        ));
    }
    println!("UTSH WebUI listening at {url}");

    // 5. 打开浏览器。
    if cfg.webui.auto_open {
        open_browser(&url);
    } else {
        println!("(auto_open = false — open {url} manually)");
    }

    // 进程保持前台运行：等待后端退出（Ctrl-C 中断即可）。
    let status = child.wait().context("WebUI backend exited unexpectedly")?;
    println!("WebUI backend stopped ({status})");
    Ok(())
}
