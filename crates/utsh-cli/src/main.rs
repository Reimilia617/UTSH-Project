use clap::Parser;

fn main() {
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
