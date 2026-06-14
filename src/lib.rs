pub mod bashrc;
pub mod config;
pub mod config_cmd;
pub mod deps;
pub mod docker;
pub mod fzf;
pub mod install;
pub mod sessionizer;
pub mod setup;
pub mod terminal_ui;
pub mod tmux;
pub mod tmux_conf;
pub mod updates;

use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("{0}")]
    Dependency(String),
    #[error("unknown arguments (try --help)")]
    UnknownArguments,
}

pub fn run_cli<I>(args: I) -> Result<(), CliError>
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();
    match args.as_slice() {
        [] => run_sessionize(),
        [cmd] if cmd == "sessionize" || cmd == "s" => run_sessionize(),
        [cmd] if cmd == "init" => run_init(),
        [cmd] if cmd == "user-config" => run_user_config(),
        [cmd, rest @ ..] if cmd == "config" => Ok(config_cmd::run(rest)?),
        [cmd, rest @ ..] if cmd == "update" => Ok(updates::run(rest)?),
        [cmd] if cmd == "__tmuxxer_update_check" => Ok(updates::run_background_check()?),
        [cmd] if cmd == "--ignore" => Ok(setup::run_ignore()?),
        [cmd, paths @ ..] if cmd == "--ignore" => Ok(setup::run_ignore_direct(paths)?),
        [cmd, paths @ ..] if cmd == "--add" => run_add_direct(paths),
        [cmd] if cmd == "--version" || cmd == "-v" => {
            print_version();
            Ok(())
        }
        [cmd] if cmd == "--help" || cmd == "-h" => {
            print_help();
            Ok(())
        }
        _ => Err(CliError::UnknownArguments),
    }
}

pub fn should_report_error(error: &CliError) -> bool {
    !matches!(error, CliError::Io(e) if e.kind() == io::ErrorKind::Interrupted)
}

fn ensure_tools() -> Result<(), CliError> {
    deps::ensure_tools().map_err(CliError::Dependency)
}

fn run_sessionize() -> Result<(), CliError> {
    ensure_tools()?;
    if !config::exists() {
        setup::run()?;
    }
    sessionizer::run()?;
    Ok(())
}

fn run_init() -> Result<(), CliError> {
    ensure_tools()?;
    setup::run()?;
    Ok(())
}

fn run_add_direct(paths: &[String]) -> Result<(), CliError> {
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "missing path (try tmuxxer --add ~/code)",
        )
        .into());
    }
    setup::run_add_direct(paths)?;
    Ok(())
}

fn run_user_config() -> Result<(), CliError> {
    ensure_tools()?;
    setup::run_user_config_setup()?;
    Ok(())
}

fn print_version() {
    println!("tmuxxer {}", env!("CARGO_PKG_VERSION"));
}

fn print_help() {
    println!("{}", help_text());
}

fn help_text() -> &'static str {
    concat!(
        "tmuxxer — tmux sessionizer\n",
        "\n",
        "Usage:\n",
        "  tmuxxer              Pick a folder, tmux session, or Docker container (fzf)\n",
        "  tmuxxer sessionize   Same as default\n",
        "  tmuxxer init         Re-run setup and rewrite config\n",
        "  tmuxxer user-config  Reconfigure tmux/bash user bindings\n",
        "  tmuxxer config path  Print config file path\n",
        "  tmuxxer config list  Print current config values\n",
        "  tmuxxer config get KEY\n",
        "                       Print a config value\n",
        "  tmuxxer config set KEY VALUE\n",
        "                       Set a config value\n",
        "  tmuxxer config toggle KEY\n",
        "                       Toggle a boolean config value\n",
        "  tmuxxer config validate\n",
        "                       Check config syntax and required values\n",
        "  tmuxxer config migrate\n",
        "                       Rewrite legacy config as TOML v2\n",
        "  tmuxxer update --check\n",
        "                       Check GitHub releases for updates\n",
        "  tmuxxer update --dismiss\n",
        "                       Hide the currently available update\n",
        "  tmuxxer --ignore     Add ignored paths or patterns\n",
        "  tmuxxer --ignore PATH...\n",
        "                       Toggle ignores without the interactive prompt\n",
        "  tmuxxer --add PATH...\n",
        "                       Toggle search roots without re-running init\n",
        "  tmuxxer --version    Print version\n",
        "\n",
        "First run: interactive setup writes config paths, then opens the picker.\n",
        "Requires tmux and fzf on PATH. Docker is optional for container entries.\n",
        "\n",
        "Config: ~/.config/tmuxxer/config (or $XDG_CONFIG_HOME/tmuxxer/config)\n",
        "  sources.docker = true     Show Docker entries in the picker\n",
        "  session.name_strategy     Directory session names: path or basename\n",
        "  docker.new_session = true Open selected Docker entries in new tmux sessions\n",
        "  [[search.roots]]          Search roots and scan depth\n",
        "  search.ignore             Ignored path or component patterns"
    )
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests;
