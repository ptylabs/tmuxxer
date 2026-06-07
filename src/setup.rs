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

    let ignores = config::Config::load()
        .map(|config| config.ignores)
        .unwrap_or_default();
    config::save(&roots, &ignores)?;

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
    if !ignores.is_empty() {
        saved_lines.push(String::new());
        saved_lines.push(format!("Preserved {} ignore(s)", ignores.len()));
    }
    ui.section("Config written", &saved_lines);

    run_user_config_setup_with_ui(&ui)?;

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
    if config.ignores.is_empty() {
        ui.note("No ignores configured.");
    } else {
        ui.section("Current ignores", &config.ignores);
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
            "tmux binding: runs tmuxxer in the current pane.",
            "bash binding: runs tmuxxer outside tmux in interactive shells.",
        ],
    );
    run_user_config_setup_with_ui(&ui)
}

fn run_user_config_setup_with_ui(ui: &TerminalUi) -> io::Result<()> {
    ui.section(
        "Key bindings",
        &[
            "Ctrl+F can open the tmuxxer picker from shell prompts in tmux and Bash.",
            "Existing tmuxxer blocks are updated in place.",
        ],
    );

    let tmux_conf = if prompt_yes_no(ui, "Add Ctrl+F binding for tmux?", true)? {
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
    };

    let bash_added = if prompt_yes_no(ui, "Add Ctrl+F binding for bash? (outside tmux only)", true)?
    {
        let already_configured = bashrc::has_ctrl_f_binding()?;
        bashrc::install_ctrl_f_binding()?;
        if already_configured {
            ui.note("Bash binding was already configured; updated it in place.");
        } else {
            ui.success("Added Bash binding.");
        }
        true
    } else {
        ui.note("Skipped Bash binding.");
        false
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
        let removed = config.ignores.remove(index);
        ToggleResult::Removed(removed)
    } else {
        config.ignores.push(normalized.to_string());
        ToggleResult::Added
    }
}

fn toggle_root(config: &mut config::Config, path: PathBuf) -> io::Result<ToggleResult> {
    let path = path.canonicalize().unwrap_or(path);
    if let Some(index) = config
        .roots
        .iter()
        .position(|root| paths_equal(&root.path, &path))
    {
        if config.roots.len() == 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot remove the only search root",
            ));
        }
        let removed = config.roots.remove(index).path;
        Ok(ToggleResult::Removed(config::stored_path(&removed)))
    } else {
        config.roots.push(SearchRoot { path, depth: 1 });
        Ok(ToggleResult::Added)
    }
}

fn find_ignore_index(config: &config::Config, home: &Path, normalized: &str) -> Option<usize> {
    config
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
    config::Config::load().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                io::ErrorKind::NotFound,
                "config not found; run tmuxxer init first",
            )
        } else {
            e
        }
    })
}

fn append_ignore(config: &mut config::Config, line: &str) -> AppendResult {
    if config.ignores.iter().any(|ignore| ignore == line) {
        return AppendResult::Duplicate;
    }
    config.ignores.push(line.to_string());
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
    let path = path.canonicalize().map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{input}: {e}"),
        )
    })?;
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
    if !line.contains('/')
        && line != "."
        && !line.starts_with('~')
        && !line.starts_with('/')
    {
        return Ok(line.to_string());
    }
    if line.contains('*') {
        return Ok(sanitize_ignore_path_pattern(line));
    }
    if line == "."
        || line.starts_with("./")
        || line.starts_with('/')
        || line.starts_with('~')
    {
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
    line.strip_prefix("./")
        .unwrap_or(line)
        .to_string()
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
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn append_ignore_rejects_duplicates() {
        let mut config = config::Config {
            roots: vec![SearchRoot {
                path: PathBuf::from("/tmp"),
                depth: 1,
            }],
            ignores: vec!["target".to_string()],
        };

        assert!(matches!(
            append_ignore(&mut config, "target"),
            AppendResult::Duplicate
        ));
        assert!(matches!(
            append_ignore(&mut config, "node_modules"),
            AppendResult::Added
        ));
    }

    #[test]
    fn toggle_ignore_adds_and_removes() {
        let home = PathBuf::from("/home/user");
        let mut config = config::Config {
            roots: vec![SearchRoot {
                path: PathBuf::from("/tmp"),
                depth: 1,
            }],
            ignores: vec!["target".to_string()],
        };

        assert!(matches!(
            toggle_ignore(&mut config, &home, "node_modules"),
            ToggleResult::Added
        ));
        assert_eq!(config.ignores.len(), 2);

        assert!(matches!(
            toggle_ignore(&mut config, &home, "target"),
            ToggleResult::Removed(_)
        ));
        assert_eq!(config.ignores, vec!["node_modules".to_string()]);
    }

    #[test]
    fn toggle_root_adds_and_removes() {
        let existing = PathBuf::from("/tmp/work");
        let extra = PathBuf::from("/tmp/other");
        let mut config = config::Config {
            roots: vec![
                SearchRoot {
                    path: existing.clone(),
                    depth: 1,
                },
                SearchRoot {
                    path: extra.clone(),
                    depth: 1,
                },
            ],
            ignores: Vec::new(),
        };

        assert!(matches!(
            toggle_root(&mut config, existing.clone()).unwrap(),
            ToggleResult::Removed(_)
        ));
        assert_eq!(config.roots.len(), 1);
        assert_eq!(config.roots[0].path, extra);

        assert!(matches!(
            toggle_root(&mut config, PathBuf::from("/tmp/new")).unwrap(),
            ToggleResult::Added
        ));
        assert_eq!(config.roots.len(), 2);
    }

    #[test]
    fn toggle_root_rejects_removing_only_root() {
        let path = PathBuf::from("/tmp/work");
        let mut config = config::Config {
            roots: vec![SearchRoot {
                path: path.clone(),
                depth: 1,
            }],
            ignores: Vec::new(),
        };

        let err = toggle_root(&mut config, path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    #[cfg(unix)]
    fn toggle_root_matches_canonical_duplicates() {
        let dir = unique_temp_dir("tmuxxer-setup-root");
        let alias = dir.join("alias");
        std::os::unix::fs::symlink(&dir, &alias).unwrap();

        let mut config = config::Config {
            roots: vec![
                SearchRoot {
                    path: dir.clone(),
                    depth: 1,
                },
                SearchRoot {
                    path: PathBuf::from("/tmp/other"),
                    depth: 1,
                },
            ],
            ignores: Vec::new(),
        };

        assert!(matches!(
            toggle_root(&mut config, alias).unwrap(),
            ToggleResult::Removed(_)
        ));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn normalize_ignore_cli_input_expands_dot_to_stored_path() {
        let dir = unique_temp_dir("tmuxxer-setup");
        let previous = env::current_dir().ok();
        env::set_current_dir(&dir).unwrap();

        let stored = normalize_ignore_cli_input(&dir, ".").unwrap();

        assert_eq!(stored, config::stored_path(&dir));

        if let Some(previous) = previous {
            let _ = env::set_current_dir(previous);
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn normalize_ignore_cli_input_keeps_patterns() {
        let home = PathBuf::from("/home/user");
        assert_eq!(
            normalize_ignore_cli_input(&home, "target").unwrap(),
            "target"
        );
        assert_eq!(
            normalize_ignore_cli_input(&home, "./folder/").unwrap(),
            "folder"
        );
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
