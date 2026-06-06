mod bashrc;
mod config;
mod deps;
mod fzf;
mod install;
mod sessionizer;
mod setup;
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
        [cmd] if cmd == "--ignore" => run_ignore(),
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

fn run_ignore() -> io::Result<()> {
    setup::run_ignore()
}

fn run_user_config() -> io::Result<()> {
    ensure_tools_or_exit();
    setup::run_user_config_setup()
}

fn print_help() {
    println!(
        "tmuxxer — tmux sessionizer\n\
         \n\
         Usage:\n\
          tmuxxer              Pick a folder or session (fzf)\n\
          tmuxxer sessionize   Same as default\n\
          tmuxxer init         Re-run setup and rewrite config\n\
          tmuxxer user-config  Reconfigure tmux/bash user bindings\n\
          tmuxxer --ignore     Add ignored paths or patterns\n\
         \n\
         First run: interactive setup writes config paths, then opens the picker.\n\
         Requires tmux and fzf on PATH.\n\
         \n\
         Config: ~/.config/tmuxxer/config (or $XDG_CONFIG_HOME/tmuxxer/config)\n\
           path = ~/code    Search root (repeatable)\n\
           depth = 1        Scan depth for the preceding path\n\
           ignore = target  Ignored path or component pattern"
    );
}
