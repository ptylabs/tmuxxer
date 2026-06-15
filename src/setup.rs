use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::bashrc;
use crate::config::{self, SearchRoot};
use crate::terminal_ui::TerminalUi;
use crate::tmux_conf;

const PRESET_SUFFIXES: &[&str] = &["code", "work", "projects", "personal", "dev"];

pub fn run() -> io::Result<()> {
    let home = config::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
    let ui = TerminalUi::new();

    ui.banner(
        "tmuxxer init",
        "Configure fast project switching for tmux.",
        &[
            "This wizard writes the project roots used by the session picker.".to_string(),
            format!("Config: {}", config::config_path().display()),
        ],
    );

    let mut paths = Vec::new();

    ui.section(
        "Project roots",
        &[
            "Pick the folders tmuxxer should scan for projects.",
            "Common development folders are detected automatically.",
        ],
    );

    if prompt_yes_no(&ui, "Add home directory (~)?", false)? {
        push_unique(&mut paths, home.clone());
    }

    // Offer detected common folders with a simple yes/no.
    let mut detected = 0usize;
    for suffix in PRESET_SUFFIXES {
        let candidate = home.join(suffix);
        if candidate.is_dir() {
            detected += 1;
        }
        if candidate.is_dir() && prompt_yes_no(&ui, &format!("Add ~/{suffix}?"), true)? {
            push_unique(&mut paths, candidate);
        }
    }
    if detected == 0 {
        ui.note("No common development folders were found.");
    }

    // Free-form additional paths.
    ui.section(
        "Extra paths",
        &[
            "Add any other folders you want to search.",
            "Examples: ~/src, ~/repos, /opt/work. Press Enter on an empty line to finish.",
        ],
    );
    loop {
        let line = prompt(&ui.input("path>"))?;
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        let path = config::expand_path(&home, line);
        if path.is_dir() {
            let path = path.canonicalize().unwrap_or(path);
            push_unique(&mut paths, path);
            ui.success(&format!("Added {line}"));
        } else {
            ui.warn(&format!("Skipped {line} (not a directory)"));
        }
    }

    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no search paths selected",
        ));
    }

    ui.section(
        "Scan depth",
        &[
            "Depth controls how far below each root tmuxxer looks.",
            "Use 1 for immediate project folders, or a larger number for nested workspaces.",
        ],
    );
    let mut roots = Vec::new();
    for path in paths {
        let label = config::stored_path(&path);
        let depth = prompt_depth(&ui, &format!("Scan depth for {label}"), 1)?;
        roots.push(SearchRoot { path, depth });
    }

    let mut next_config = match config::Config::load() {
        Ok(config) => config.into_inner(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => config::Config::default(),
        Err(e) => return Err(e.into()),
    };
    let preserved_ignores = next_config.search.ignores.len();
    next_config.search.roots = roots.clone();
    next_config.sources.directories = true;
    next_config.save()?;

    let mut saved_lines = vec![
        format!(
            "Saved {} path(s) to {}",
            roots.len(),
            config::config_path().display()
        ),
        String::new(),
    ];
    for root in &roots {
        saved_lines.push(format!(
            "- {} (depth {})",
            config::stored_path(&root.path),
            root.depth
        ));
    }
    if preserved_ignores > 0 {
        saved_lines.push(String::new());
        saved_lines.push(format!("Preserved {preserved_ignores} ignore(s)"));
    }
    ui.section("Config written", &saved_lines);

    run_optional_user_config_setup_with_ui(&ui)?;

    Ok(())
}

pub fn run_ignore() -> io::Result<()> {
    let ui = TerminalUi::new();
    let mut config = load_config()?;

    ui.banner(
        "tmuxxer ignore",
        "Add paths or patterns the picker should skip.",
        &[format!("Config: {}", config::config_path().display())],
    );
    if config.search.ignores.is_empty() {
        ui.note("No ignores configured.");
    } else {
        ui.section("Current ignores", &config.search.ignores);
    }

    ui.section(
        "Add ignores",
        &[
            "Press Enter on an empty line to finish.",
            "Examples: target, .*, node_modules/*, ~/work/tmp",
        ],
    );
    let mut added = 0usize;
    loop {
        let line = prompt(&ui.input("ignore>"))?;
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        match append_ignore(&mut config, line) {
            AppendResult::Added => {
                added += 1;
                ui.success(&format!("Added {line}"));
            }
            AppendResult::Duplicate => ui.warn(&format!("Skipped {line} (already ignored)")),
        }
    }

    if added > 0 {
        config.save()?;
    }

    ui.section(
        "Config written",
        &[format!(
            "Saved {added} new ignore(s) to {}",
            config::config_path().display()
        )],
    );
    Ok(())
}

pub fn run_ignore_direct(paths: &[String]) -> io::Result<()> {
    let home = config::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
    let mut config = load_config()?;
    let mut changed = false;

    for path in paths {
        let line = path.trim();
        if line.is_empty() {
            continue;
        }
        let line = normalize_ignore_cli_input(&home, line)?;
        match toggle_ignore(&mut config, &home, &line) {
            ToggleResult::Added => {
                changed = true;
                println!("Added {line}");
            }
            ToggleResult::Removed(removed) => {
                changed = true;
                println!("Removed {removed}");
            }
        }
    }

    if changed {
        config.save()?;
    }
    Ok(())
}

pub fn run_add_direct(paths: &[String]) -> io::Result<()> {
    let home = config::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;
    let mut config = load_config()?;
    let mut changed = false;

    for path in paths {
        let line = path.trim();
        if line.is_empty() {
            continue;
        }
        let resolved = resolve_directory(&home, line)?;
        let label = config::stored_path(&resolved);
        match toggle_root(&mut config, resolved)? {
            ToggleResult::Added => {
                changed = true;
                println!("Added {label}");
            }
            ToggleResult::Removed(removed) => {
                changed = true;
                println!("Removed {removed}");
            }
        }
    }

    if changed {
        config.save()?;
    }
    Ok(())
}

pub fn run_user_config_setup() -> io::Result<()> {
    let ui = TerminalUi::new();
    ui.banner(
        "tmuxxer user-config",
        "Install optional Ctrl+F shortcuts for the picker.",
        &[
            "bash binding: runs tmuxxer in interactive Bash shells.",
            "tmux passthrough: forwards Ctrl+F to the current pane.",
            "Docker entries: choose picker visibility and opening behavior.",
        ],
    );
    run_user_config_setup_with_ui(&ui, true)
}

fn run_optional_user_config_setup_with_ui(ui: &TerminalUi) -> io::Result<()> {
    let result = run_user_config_setup_with_ui(ui, false);
    handle_optional_user_config_setup_result(ui, result)
}

fn handle_optional_user_config_setup_result(
    ui: &TerminalUi,
    result: io::Result<()>,
) -> io::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::Interrupted => Err(e),
        Err(e) => {
            ui.warn(&format!("Optional key binding setup did not complete: {e}"));
            ui.note("Run tmuxxer user-config later to retry optional key bindings.");
            Ok(())
        }
    }
}

fn run_user_config_setup_with_ui(ui: &TerminalUi, include_docker_config: bool) -> io::Result<()> {
    ui.section(
        "Key bindings",
        &[
            "Ctrl+F can open the tmuxxer picker from shell prompts in tmux and Bash.",
            "Inside tmux, Bash handles the picker so the command is not typed into the pane.",
            "Existing tmuxxer blocks are updated in place.",
        ],
    );

    let bash_was_configured = bashrc::has_ctrl_f_binding()?;
    let bash_added = if prompt_yes_no(ui, "Add Ctrl+F binding for bash?", true)? {
        bashrc::install_ctrl_f_binding()?;
        if bash_was_configured {
            ui.note("Bash binding was already configured; updated it in place.");
        } else {
            ui.success("Added Bash binding.");
        }
        true
    } else {
        ui.note("Skipped Bash binding.");
        false
    };
    let bash_available = bash_added || bash_was_configured;

    let tmux_conf = if bash_available {
        if prompt_yes_no(ui, "Add Ctrl+F tmux passthrough?", true)? {
            let already_configured = tmux_conf::has_ctrl_f_binding()?;
            let conf = tmux_conf::install_ctrl_f_binding()?;
            if already_configured {
                ui.note("tmux binding was already configured; updated it in place.");
            } else {
                ui.success("Added tmux binding.");
            }
            Some(conf)
        } else {
            ui.note("Skipped tmux binding.");
            None
        }
    } else {
        ui.note("Skipped tmux binding because it only forwards Ctrl+F to a shell binding.");
        None
    };

    if let Some(conf) = tmux_conf {
        if prompt_yes_no(ui, "Reload tmux config now?", true)? {
            match tmux_conf::reload_config(&conf) {
                Ok(()) => ui.success("Reloaded tmux config."),
                Err(e) => {
                    ui.warn(&format!("Could not reload tmux config now: {e}"));
                    ui.note(&format!(
                        "Run this manually: tmux source-file {}",
                        conf.display()
                    ));
                }
            }
        } else {
            ui.note(&format!(
                "Reload tmux later: tmux source-file {}",
                conf.display()
            ));
        }
    }

    if bash_added {
        ui.section(
            "Shell note",
            &[
                "Bash Ctrl+F is active in new interactive shells.",
                "For this shell, run: source ~/.bashrc",
            ],
        );
    }

    if include_docker_config {
        run_docker_config_setup(ui)?;
    }

    Ok(())
}

fn run_docker_config_setup(ui: &TerminalUi) -> io::Result<()> {
    let mut config = match load_config() {
        Ok(config) => config,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            ui.warn("Docker behavior needs a tmuxxer config file.");
            ui.note("Run tmuxxer init first, or edit the config file after it exists.");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    ui.section(
        "Docker entries",
        &[
            "Choose whether running containers appear in the picker.",
            "Opening behavior only applies after selecting a Docker entry.",
        ],
    );

    let show_docker = prompt_yes_no(
        ui,
        "Show Docker containers in picker?",
        config.sources.docker,
    )?;
    let use_new_session = prompt_yes_no(
        ui,
        "Open Docker containers in new tmux sessions?",
        config.docker.new_session,
    )?;

    if config.sources.docker == show_docker && config.docker.new_session == use_new_session {
        ui.note("Docker settings are already up to date.");
        return Ok(());
    }

    config.sources.docker = show_docker;
    config.docker.new_session = use_new_session;
    config.save()?;
    if show_docker {
        let label = if use_new_session {
            "new tmux session"
        } else {
            "current pane"
        };
        ui.success(&format!(
            "Docker entries will show and open in the {label}."
        ));
    } else {
        ui.success("Docker entries will be hidden from the picker.");
    }

    Ok(())
}

enum AppendResult {
    Added,
    Duplicate,
}

#[derive(Debug)]
enum ToggleResult {
    Added,
    Removed(String),
}

fn toggle_ignore(config: &mut config::Config, home: &Path, normalized: &str) -> ToggleResult {
    if let Some(index) = find_ignore_index(config, home, normalized) {
        let removed = config.search.ignores.remove(index);
        ToggleResult::Removed(removed)
    } else {
        config.search.ignores.push(normalized.to_string());
        ToggleResult::Added
    }
}

fn toggle_root(config: &mut config::Config, path: PathBuf) -> io::Result<ToggleResult> {
    let path = path.canonicalize().unwrap_or(path);
    if let Some(index) = config
        .search
        .roots
        .iter()
        .position(|root| paths_equal(&root.path, &path))
    {
        if config.search.roots.len() == 1 && config.sources.directories {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot remove the only search root while sources.directories is true",
            ));
        }
        let removed = config.search.roots.remove(index).path;
        Ok(ToggleResult::Removed(config::stored_path(&removed)))
    } else {
        config.search.roots.push(SearchRoot { path, depth: 1 });
        Ok(ToggleResult::Added)
    }
}

fn find_ignore_index(config: &config::Config, home: &Path, normalized: &str) -> Option<usize> {
    config
        .search
        .ignores
        .iter()
        .position(|entry| ignore_entries_match(home, entry, normalized))
}

fn ignore_entries_match(home: &Path, left: &str, right: &str) -> bool {
    if left == right {
        return true;
    }
    match (
        normalize_ignore_cli_input(home, left),
        normalize_ignore_cli_input(home, right),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn load_config() -> io::Result<config::Config> {
    config::Config::load()
        .map(|config| config.into_inner())
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "config not found; run tmuxxer init first",
                )
            } else {
                e.into()
            }
        })
}

fn append_ignore(config: &mut config::Config, line: &str) -> AppendResult {
    if config.search.ignores.iter().any(|ignore| ignore == line) {
        return AppendResult::Duplicate;
    }
    config.search.ignores.push(line.to_string());
    AppendResult::Added
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn resolve_directory(home: &Path, input: &str) -> io::Result<PathBuf> {
    let path = resolve_user_path(home, input)?;
    let path = path
        .canonicalize()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("{input}: {e}")))?;
    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{input}: not a directory"),
        ));
    }
    Ok(path)
}

fn normalize_ignore_cli_input(home: &Path, line: &str) -> io::Result<String> {
    let line = line.trim();
    if !line.contains('/') && line != "." && !line.starts_with('~') && !line.starts_with('/') {
        return Ok(line.to_string());
    }
    if line.contains('*') {
        return Ok(sanitize_ignore_path_pattern(line));
    }
    if line == "." || line.starts_with("./") || line.starts_with('/') || line.starts_with('~') {
        let path = resolve_user_path(home, line)?;
        if path.is_dir() {
            if let Ok(path) = path.canonicalize() {
                return Ok(config::stored_path(&path));
            }
        }
    }
    Ok(sanitize_ignore_path_pattern(line))
}

fn sanitize_ignore_path_pattern(line: &str) -> String {
    let line = line.trim().trim_end_matches('/');
    line.strip_prefix("./").unwrap_or(line).to_string()
}

fn resolve_user_path(home: &Path, input: &str) -> io::Result<PathBuf> {
    let input = input.trim();
    if input == "." {
        return env::current_dir();
    }
    if input.starts_with("./") {
        return Ok(env::current_dir()?.join(input.strip_prefix("./").unwrap_or(input)));
    }
    if !input.starts_with('~') && !Path::new(input).is_absolute() {
        return Ok(env::current_dir()?.join(input));
    }
    Ok(config::expand_path(home, input))
}

fn prompt(label: &str) -> io::Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut buf = String::new();
    let stdin = io::stdin();
    let stdin_is_terminal = stdin.is_terminal();
    let n = stdin.read_line(&mut buf)?;
    if !stdin_is_terminal {
        println!();
    }
    if n == 0 {
        // EOF (Ctrl-D): treat as empty / finish.
        return Ok(String::new());
    }
    Ok(buf)
}

fn prompt_yes_no(ui: &TerminalUi, question: &str, default_yes: bool) -> io::Result<bool> {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    let answer = prompt(&ui.question(question, hint))?;
    let answer = answer.trim().to_lowercase();
    if answer.is_empty() {
        return Ok(default_yes);
    }
    Ok(matches!(answer.as_str(), "y" | "yes"))
}

fn prompt_depth(ui: &TerminalUi, question: &str, default: usize) -> io::Result<usize> {
    let answer = prompt(&ui.question(question, &format!("[{default}]:")))?;
    let answer = answer.trim();
    if answer.is_empty() {
        return Ok(default);
    }
    Ok(answer.parse::<usize>().unwrap_or(default).max(1))
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|p| p == &path) {
        paths.push(path);
    }
}

#[cfg(test)]
mod tests;
