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
        let path = expand(&home, line);
        if path.is_dir() {
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
        let label = display_path(&home, &path);
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
            display_path(&home, &root.path),
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
    let mut config = config::Config::load().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                io::ErrorKind::NotFound,
                "config not found; run tmuxxer init first",
            )
        } else {
            e
        }
    })?;

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
        if config.ignores.iter().any(|ignore| ignore == line) {
            ui.warn(&format!("Skipped {line} (already ignored)"));
        } else {
            config.ignores.push(line.to_string());
            added += 1;
            ui.success(&format!("Added {line}"));
        }
    }

    if added > 0 {
        config::save(&config.roots, &config.ignores)?;
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

fn expand(home: &Path, input: &str) -> PathBuf {
    if input == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = input.strip_prefix("~/") {
        return home.join(rest);
    }
    PathBuf::from(input)
}

fn display_path(home: &Path, path: &Path) -> String {
    if path == home {
        return "~".to_string();
    }
    if let Ok(rest) = path.strip_prefix(home) {
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|p| p == &path) {
        paths.push(path);
    }
}
