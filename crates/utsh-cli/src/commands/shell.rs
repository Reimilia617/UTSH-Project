//! 最小可用交互式 Shell 会话（让 `utsh` 可被 `chsh` 设为登录 shell）。
//!
//! 能力（当前最小集，随 Phase 1/2 演进）：
//! - 登录 shell：argv[0] 形如 `-utsh` 时自动进入；或显式 `utsh shell`；
//! - `-c <cmd>` 一次性执行；`utsh shell <file>` 按行执行脚本；
//! - 非 TTY 标准输入（管道）按脚本逐行执行；
//! - 每行先做**别名展开**（读用户配置 `~/.config/ut/utsh.toml`），跳过空行/`#`；
//! - 内建：`exit [n]`、`cd`、`pwd`、`help`；
//! - 其余命令：由 C++ 解析器产出 argv 后直接 `spawn`（`/bin/sh -c` 仅在语法暂
//!   不支持时兜底），stdin/stdout/stderr 透传，退出码返回给调用者；
//! - 交互会话把执行过的行追加写入 `~/.local/share/utsh/history`。
//!
//! 本模块刻意保持“stdio 行循环 + 无 raw 终端”，先让登录 shell 可用、可测；
//! ZLE/终端层的接入在后续 Phase 落地（`crates/utsh-core/src/zle` 已就绪）。

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use utsh_core::alias::AliasTable;
use utsh_core::history::History;
use utsh_core::parser::{self, ParseOutcome};

use super::resolve_config;

/// `utsh shell` 子命令参数（亦用于登录 shell 入口）。
#[derive(Debug, Clone, Default)]
pub struct ShellArgs {
    /// `-c` 一次性执行的命令。
    pub command: Option<String>,
    /// 脚本文件路径。
    pub file: Option<PathBuf>,
    /// 配置覆盖（`--config` / `UTSH_CONFIG`），沿用 CLI 语义。
    pub config: Option<PathBuf>,
}

/// 会话上下文。
struct Session {
    aliases: AliasTable,
    history: Option<History>,
    last_code: i32,
}

impl Session {
    fn expand(&self, line: &str) -> String {
        self.aliases.expand(line)
    }
}

/// 打开配置文件与历史文件（历史仅在交互会话写盘）。
fn open_session(config_override: Option<&Path>) -> anyhow::Result<(Session, PathBuf)> {
    let (_path, cfg) = resolve_config(&config_override.map(Path::to_path_buf))?;
    let aliases =
        AliasTable::from_entries(cfg.aliases().iter().map(|(k, v)| (k.clone(), v.clone())));
    let hist_path = utsh_core::config::Config::data_dir().join("history");
    let history = History::load(
        &hist_path,
        cfg.general.history_size,
        cfg.general.history_ignore_dups,
    )
    .ok();
    Ok((
        Session {
            aliases,
            history,
            last_code: 0,
        },
        hist_path,
    ))
}

/// 把文本追加进历史文件（interactive 会话用）。
fn append_history(session: &mut Session, hist_path: &Path, line: &str) {
    let Some(hist) = session.history.as_mut() else {
        return;
    };
    if hist.push(line.to_string()) {
        if let Some(parent) = hist_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(hist_path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// 行内是否含 shell 元字符（引号感知）。命中则整行交给 `/bin/sh -c`：
/// 迷你 C++ 解析器只支持简单命令，管道/重定向/`&&`/赋值等仍需系统 shell。
fn contains_shell_meta(line: &str) -> bool {
    // 变量/命令替换（`$`、反引号）无论是否引号都交给 sh（引号内也需展开）。
    if line.contains('$') || line.contains('`') {
        return true;
    }
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    for c in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            c if !in_single
                && !in_double
                && matches!(c, ';' | '|' | '&' | '<' | '>' | '(' | ')' | '=') =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// 单行执行：内建 / 外部命令 / sh 兜底。返回退出码。
fn execute_line(line: &str, session: &mut Session) -> i32 {
    let line = session.expand(line.trim());
    if line.is_empty() || line.starts_with('#') {
        return session.last_code;
    }

    // ---- 内建 ----
    if let Some(rest) = line.strip_prefix("exit") {
        let code = rest.trim().parse::<i32>().unwrap_or(0);
        return code; // 由调用方据此终止会话
    }
    if line == "cd" {
        if let Some(home) = std::env::var_os("HOME") {
            let ok = std::env::set_current_dir(home);
            return if ok.is_ok() { 0 } else { 1 };
        }
        return 0;
    }
    if let Some(dir) = line.strip_prefix("cd ") {
        let dir = dir.trim();
        let dir = if dir.starts_with("~/") {
            std::env::var("HOME")
                .map(|h| format!("{h}{}", &dir[1..]))
                .unwrap_or_else(|_| dir.to_string())
        } else {
            dir.to_string()
        };
        return match std::env::set_current_dir(&dir) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("utsh: cd: {dir}: {e}");
                1
            }
        };
    }
    if line == "pwd" {
        if let Ok(p) = std::env::current_dir() {
            println!("{}", p.display());
            return 0;
        }
        return 1;
    }
    if line == "help" {
        println!("UTSH shell (minimal)");
        println!("  builtins: exit [n], cd [dir], pwd, help");
        println!(
            "  syntax: simple commands with quotes/escapes; fallback to sh for pipes/redirection"
        );
        return 0;
    }
    if line == "history" {
        if let Some(hist) = &session.history {
            for (i, e) in hist.entries().iter().enumerate() {
                println!("{:>5}  {}", i + 1, e);
            }
        }
        return 0;
    }

    // ---- 解析 → 外部命令 ----
    // 含 shell 元字符（管道/重定向/&&/赋值等，引号感知）→ 整行交给系统 shell。
    if contains_shell_meta(&line) {
        return sh_run(&line);
    }
    match parser::parse(&line) {
        ParseOutcome::Parsed(node) if node.is_command() => {
            let children = &node.children;
            if children.is_empty() {
                return 0;
            }
            match Command::new(&children[0]).args(&children[1..]).status() {
                Ok(st) => st.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("utsh: {}: {e}", children[0]);
                    127
                }
            }
        }
        _ => sh_run(&line),
    }
}

/// 语法暂不支持（或含元字符）时交给 `/bin/sh -c` 执行。
fn sh_run(line: &str) -> i32 {
    match Command::new("/bin/sh").arg("-c").arg(line).status() {
        Ok(st) => st.code().unwrap_or(1),
        Err(e) => {
            eprintln!("utsh: failed to run via /bin/sh: {e}");
            1
        }
    }
}

/// `exit` 的结果是否终止会话（`exit` 或 `exit <数字>`）。
fn should_exit(line: &str) -> bool {
    let trimmed = line.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    match (parts.next(), parts.next()) {
        (Some("exit"), None) => true,
        (Some("exit"), Some(rest)) => rest.trim().parse::<i32>().is_ok(),
        _ => false,
    }
}

/// 主入口。`-c`/脚本/管道/交互四种形态共享实现。
pub fn run(mut args: ShellArgs) -> anyhow::Result<i32> {
    let (mut session, hist_path) = open_session(args.config.as_deref())?;

    // 1) -c
    if let Some(cmd) = args.command.take() {
        let code = execute_line(&cmd, &mut session);
        return Ok(code);
    }
    // 2) 脚本文件
    if let Some(file) = args.file.take() {
        let content = std::fs::read_to_string(&file)
            .with_context(|| format!("utsh: cannot read script {}", file.display()))?;
        let mut last = 0;
        for line in content.lines() {
            last = execute_line(line, &mut session);
            if should_exit(line) {
                break;
            }
        }
        return Ok(last);
    }

    // 3) stdin 非 TTY：把 stdin 当脚本逐行执行（无提示符）
    if !std::io::stdin().is_terminal() {
        let stdin = std::io::stdin();
        let mut last = 0;
        for line in stdin.lock().lines() {
            let line = line?;
            last = execute_line(&line, &mut session);
            if should_exit(&line) {
                break;
            }
        }
        return Ok(last);
    }

    // 4) 交互会话：raw 行编辑器（方向键历史/光标移动/灰色建议）驱动
    let lines = session
        .history
        .as_ref()
        .map(|h| h.entries().to_vec())
        .unwrap_or_default();
    let code = super::edit::run_interactive(lines, build_prompt, |line| {
        let code = execute_line(line, &mut session);
        session.last_code = code;
        append_history(&mut session, &hist_path, line.trim_end());
        code
    })?;
    Ok(code)
}

/// 提示符：绿色短路径 + `$`（root 用 `#`），如 `~/project$ `。
fn build_prompt() -> String {
    use std::path::Path;
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "?".to_string());
    let home = std::env::var("HOME").unwrap_or_default();
    let short = if !home.is_empty() && cwd == home {
        "~".to_string()
    } else if !home.is_empty() && cwd.starts_with(&home) {
        format!("~{}", &cwd[home.len()..])
    } else {
        Path::new(&cwd)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or(cwd.clone())
    };
    let no_color = std::env::var_os("NO_COLOR").is_some();
    if no_color {
        format!("{short}$ ")
    } else {
        format!("\x1b[1;32m{short}\x1b[0m$ ")
    }
}

/// 登录 shell 入口：解析 argv[0] 之后的参数（`-c cmd`、脚本路径、其余忽略）。
/// 返回 `(ShellArgs, 是否应当直接退出管理入口)` —— 简化：直接在这里跑完会话。
pub fn run_login_shell(rest: Vec<String>) -> anyhow::Result<i32> {
    let mut args = ShellArgs::default();
    let mut i = 0;
    while i < rest.len() {
        match rest[i].as_str() {
            "-c" | "--command" => {
                i += 1;
                args.command = rest.get(i).cloned();
            }
            "-l" | "--login" | "--noprofile" | "--norc" | "-i" => { /* 兼容选项：忽略 */ }
            s if s.starts_with('-') => { /* 未知选项忽略（保持 bash 宽容性） */ }
            file => args.file = Some(PathBuf::from(file)),
        }
        i += 1;
    }
    run(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_expansion_via_session() {
        let s = Session {
            aliases: AliasTable::from_entries(vec![(
                "ll".to_string(),
                "echo aliased-ll".to_string(),
            )]),
            history: None,
            last_code: 0,
        };
        assert_eq!(s.expand("ll /tmp"), "echo aliased-ll /tmp");
    }

    #[test]
    fn exit_detection() {
        assert!(should_exit("exit"));
        assert!(should_exit("  exit 3 "));
        assert!(!should_exit("exit code is fine"));
        assert!(!should_exit("echo exit"));
    }

    #[test]
    fn login_arg_parse() {
        // 模拟登录 shell：argv0 已被剥掉，剩下 “-c echo hi”
        let cmd = run_login_shell(vec!["-c".into(), "echo hi".into()]).expect("ok");
        // run() 已把 -c 当一次性命令执行（echo 为外部命令）——返回码应为 0。
        assert_eq!(cmd, 0);
    }
}
