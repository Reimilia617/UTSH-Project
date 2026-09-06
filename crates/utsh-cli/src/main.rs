use clap::Parser;
use std::path::Path;

fn main() {
    // 登录 shell：`chsh -s /usr/bin/utsh` 后，登录时系统以 argv[0]="-utsh"
    // 调用本程序，此时直接进入交互 Shell 会话（bash/zsh 的登录惯例）。
    if let Some(argv0) = std::env::args_os().next() {
        let name = Path::new(&argv0)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('-') {
            let rest: Vec<String> = std::env::args().skip(1).collect();
            let code = match utsh_cli::commands::shell::run_login_shell(rest) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("utsh: error: {e:#}");
                    1
                }
            };
            std::process::exit(code);
        }
    }

    let cli = utsh_cli::cli::Cli::parse();
    utsh_cli::cli::init_tracing(cli.verbose);
    match utsh_cli::run(cli) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("utsh: error: {e:#}");
            std::process::exit(1);
        }
    }
}
