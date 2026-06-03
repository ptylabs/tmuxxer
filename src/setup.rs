use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::config;

const PRESET_SUFFIXES: &[&str] = &["code", "work", "projects", "personal", "dev"];

pub fn run() -> io::Result<()> {
    let home = config::home_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;

    println!("tmuxxer setup");
    println!("Choose the folders to search for projects. ~ is always included.\n");

    let mut paths = vec![home.clone()];

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

    let depth = prompt_depth(1)?;

    config::save(&paths, depth)?;

    println!("\nSaved {} path(s) to {}", paths.len(), config::config_path().display());
    for p in &paths {
        println!("  {}", display_path(&home, p));
    }
    println!();

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

fn prompt_depth(default: usize) -> io::Result<usize> {
    let answer = prompt(&format!("\nScan depth under each path [{default}]: "))?;
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
