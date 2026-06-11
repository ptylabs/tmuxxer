mod bashrc;
mod config;
mod config_cmd;
mod deps;
mod docker;
mod fzf;
mod install;
mod sessionizer;
mod setup;
mod terminal_ui;
mod tmux;
mod tmux_conf;

use std::env;
use std::io;
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.as_slice() {
        [] => run_sessionize(),
        [cmd] if cmd == "sessionize" || cmd == "s" => run_sessionize(),
        [cmd] if cmd == "init" => run_init(),
        [cmd] if cmd == "user-config" => run_user_config(),
        [cmd, rest @ ..] if cmd == "config" => config_cmd::run(rest),
        [cmd] if cmd == "--ignore" => setup::run_ignore(),
        [cmd, paths @ ..] if cmd == "--ignore" => setup::run_ignore_direct(paths),
        [cmd, paths @ ..] if cmd == "--add" => run_add_direct(paths),
        [cmd] if cmd == "--version" || cmd == "-v" => {
            print_version();
            Ok(())
        }
        [cmd] if cmd == "--help" || cmd == "-h" => {
            print_help();
            Ok(())
        }
        _ => {
            eprintln!("tmuxxer: unknown arguments (try --help)");
            process::exit(1);
        }
    };

    if let Err(e) = result {
        if e.kind() != io::ErrorKind::Interrupted {
            eprintln!("tmuxxer: {e}");
        }
        process::exit(1);
    }
}

fn ensure_tools_or_exit() {
    if let Err(msg) = deps::ensure_tools() {
        eprintln!("{msg}");
        process::exit(1);
    }
}

fn run_sessionize() -> io::Result<()> {
    ensure_tools_or_exit();
    if !config::exists() {
        setup::run()?;
    }
    sessionizer::run()
}

fn run_init() -> io::Result<()> {
    ensure_tools_or_exit();
    setup::run()
}

fn run_add_direct(paths: &[String]) -> io::Result<()> {
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing path (try tmuxxer --add ~/code)",
        ));
    }
    setup::run_add_direct(paths)
}

fn run_user_config() -> io::Result<()> {
    ensure_tools_or_exit();
    setup::run_user_config_setup()
}

fn print_version() {
    println!("tmuxxer {}", env!("CARGO_PKG_VERSION"));
}

fn print_help() {
    println!(
        "tmuxxer — tmux sessionizer\n\
         \n\
         Usage:\n\
          tmuxxer              Pick a folder, tmux session, or Docker container (fzf)\n\
          tmuxxer sessionize   Same as default\n\
          tmuxxer init         Re-run setup and rewrite config\n\
          tmuxxer user-config  Reconfigure tmux/bash user bindings\n\
          tmuxxer config path  Print config file path\n\
          tmuxxer config list  Print current config values\n\
          tmuxxer config get KEY\n\
                               Print a config value\n\
          tmuxxer config set KEY VALUE\n\
                               Set a config value\n\
          tmuxxer config toggle KEY\n\
                               Toggle a boolean config value\n\
          tmuxxer config migrate\n\
                               Rewrite legacy config as TOML v2\n\
          tmuxxer --ignore     Add ignored paths or patterns\n\
          tmuxxer --ignore PATH...\n\
                               Toggle ignores without the interactive prompt\n\
          tmuxxer --add PATH...\n\
                               Toggle search roots without re-running init\n\
          tmuxxer --version    Print version\n\
         \n\
         First run: interactive setup writes config paths, then opens the picker.\n\
         Requires tmux and fzf on PATH. Docker is optional for container entries.\n\
         \n\
         Config: ~/.config/tmuxxer/config (or $XDG_CONFIG_HOME/tmuxxer/config)\n\
           sources.docker = true     Show Docker entries in the picker\n\
           session.name_strategy     Directory session names: path or basename\n\
           docker.new_session = true Open selected Docker entries in new tmux sessions\n\
           [[search.roots]]          Search roots and scan depth\n\
           search.ignore             Ignored path or component patterns"
    );
}
