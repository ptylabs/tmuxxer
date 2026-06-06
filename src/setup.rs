use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::bashrc;
use crate::config::{self, SearchRoot};
use crate::tmux_conf;

const PRESET_SUFFIXES: &[&str] = &["code", "work", "projects", "personal", "dev"];

pub fn run() -> io::Result<()> {
    let home = config::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;

    println!("tmuxxer setup");
    println!("Choose the folders to search for projects.\n");

    let mut paths = Vec::new();

    if prompt_yes_no("Add ~?", false)? {
        push_unique(&mut paths, home.clone());
    }

    // Offer detected common folders with a simple yes/no.
    for suffix in PRESET_SUFFIXES {
        let candidate = home.join(suffix);
        if candidate.is_dir() && prompt_yes_no(&format!("Add ~/{suffix}?"), true)? {
            push_unique(&mut paths, candidate);
        }
    }

    // Free-form additional paths.
    println!("\nAdd more paths (e.g. ~/src or /opt/work). Press Enter on an empty line to finish.");
    loop {
        let line = prompt("  path> ")?;
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        let path = expand(&home, line);
        if path.is_dir() {
            push_unique(&mut paths, path);
        } else {
            println!("  skipped (not a directory): {line}");
        }
    }

    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no search paths selected",
        ));
    }

    println!("\nChoose scan depth for each path.");
    let mut roots = Vec::new();
    for path in paths {
        let label = display_path(&home, &path);
        let depth = prompt_depth(&format!("Scan depth for {label}"), 1)?;
        roots.push(SearchRoot { path, depth });
    }

    let ignores = config::Config::load()
        .map(|config| config.ignores)
        .unwrap_or_default();
    config::save(&roots, &ignores)?;

    println!(
        "\nSaved {} path(s) to {}",
        roots.len(),
        config::config_path().display()
    );
    for root in &roots {
        println!(
            "  {} (depth {})",
            display_path(&home, &root.path),
            root.depth
        );
    }
    if !ignores.is_empty() {
        println!("Preserved {} ignore(s)", ignores.len());
    }
    println!();

    run_user_config_setup()?;

    Ok(())
}

pub fn run_ignore() -> io::Result<()> {
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

    println!("tmuxxer ignore");
    if config.ignores.is_empty() {
        println!("No ignores configured.");
    } else {
        println!("Current ignores:");
        for ignore in &config.ignores {
            println!("  {ignore}");
        }
    }

    println!("\nAdd ignore patterns or paths. Press Enter on an empty line to finish.");
    println!("Examples: target, .*, node_modules/*, ~/work/tmp");
    let mut added = 0usize;
    loop {
        let line = prompt("  ignore> ")?;
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if config.ignores.iter().any(|ignore| ignore == line) {
            println!("  skipped (already ignored): {line}");
        } else {
            config.ignores.push(line.to_string());
            added += 1;
        }
    }

    if added > 0 {
        config::save(&config.roots, &config.ignores)?;
    }

    println!(
        "\nSaved {added} new ignore(s) to {}",
        config::config_path().display()
    );
    Ok(())
}

pub fn run_user_config_setup() -> io::Result<()> {
    println!("tmuxxer user-config");

    let tmux_conf = if prompt_yes_no("Add Ctrl+F binding for tmux?", true)? {
        let already_configured = tmux_conf::has_ctrl_f_binding()?;
        let conf = tmux_conf::install_ctrl_f_binding()?;
        if already_configured {
            println!("Already configured");
        } else {
            println!("Added");
        }
        Some(conf)
    } else {
        println!("Not added");
        None
    };

    let bash_added = if prompt_yes_no("Add Ctrl+F binding for bash? (outside tmux only)", true)? {
        let already_configured = bashrc::has_ctrl_f_binding()?;
        bashrc::install_ctrl_f_binding()?;
        if already_configured {
            println!("Already configured");
        } else {
            println!("Added");
        }
        true
    } else {
        println!("Not added");
        false
    };

    if let Some(conf) = tmux_conf {
        if prompt_yes_no("Reload tmux config now?", true)? {
            match tmux_conf::reload_config(&conf) {
                Ok(()) => println!("Reloaded tmux config"),
                Err(e) => {
                    println!("Could not reload tmux config now: {e}");
                    println!("Run this manually: tmux source-file {}", conf.display());
                }
            }
        } else {
            println!("Reload tmux later: tmux source-file {}", conf.display());
        }
    }

    if bash_added {
        println!("Bash Ctrl+F is active in new interactive shells.");
        println!("For this shell, run: source ~/.bashrc");
    }

    Ok(())
}

fn prompt(label: &str) -> io::Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut buf = String::new();
    let n = io::stdin().read_line(&mut buf)?;
    if n == 0 {
        // EOF (Ctrl-D): treat as empty / finish.
        return Ok(String::new());
    }
    Ok(buf)
}

fn prompt_yes_no(question: &str, default_yes: bool) -> io::Result<bool> {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    let answer = prompt(&format!("{question} {hint} "))?;
    let answer = answer.trim().to_lowercase();
    if answer.is_empty() {
        return Ok(default_yes);
    }
    Ok(matches!(answer.as_str(), "y" | "yes"))
}

fn prompt_depth(question: &str, default: usize) -> io::Result<usize> {
    let answer = prompt(&format!("{question} [{default}]: "))?;
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
