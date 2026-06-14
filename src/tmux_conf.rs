use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config;

const MARKER_START: &str = "# >>> tmuxxer >>>";
const MARKER_END: &str = "# <<< tmuxxer <<<";

/// User config file tmux is loading, or a conservative default for new installs.
pub fn active_config_path() -> PathBuf {
    if let Ok(path) = env::var("TMUX_CONF") {
        return PathBuf::from(path);
    }

    if let Some(path) = loaded_user_config_path() {
        return path;
    }

    let candidates = user_config_candidates();
    if let Some(path) = candidates.iter().rev().find(|path| path.exists()) {
        return path.clone();
    }

    config::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".tmux.conf")
}

pub fn install_ctrl_f_binding() -> io::Result<PathBuf> {
    let path = active_config_path();
    for legacy_path in user_config_paths(&path) {
        if legacy_path == path || !legacy_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&legacy_path)?;
        let cleaned = remove_legacy_ctrl_f_bindings(&content);
        if cleaned != content {
            fs::write(&legacy_path, cleaned)?;
        }
    }

    let mut content = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        String::new()
    };
    content = remove_legacy_ctrl_f_bindings(&content);
    let bind_line = forward_ctrl_f_bind_line();
    let block = format!("{MARKER_START}\n{bind_line}\n{MARKER_END}\n");

    if let Some((start, end)) = find_block_span(&content) {
        content.replace_range(start..end, &block);
    } else if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
        content.push_str(&block);
    } else {
        content.push_str(&block);
    }

    fs::write(&path, content)?;
    Ok(path)
}

pub fn reload_config(path: &Path) -> io::Result<()> {
    let status = Command::new("tmux").arg("source-file").arg(path).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "tmux source-file failed for {}",
            path.display()
        )))
    }
}

fn loaded_user_config_path() -> Option<PathBuf> {
    loaded_user_config_paths().into_iter().next_back()
}

fn loaded_user_config_paths() -> Vec<PathBuf> {
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#{config_files}"])
        .output()
        .ok();
    let Some(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .filter(|path| !path.starts_with("/etc") && path.is_file())
        .collect()
}

fn user_config_paths(active_path: &Path) -> Vec<PathBuf> {
    let mut paths = loaded_user_config_paths();
    paths.extend(
        user_config_candidates()
            .into_iter()
            .filter(|path| path.exists()),
    );
    paths.push(active_path.to_path_buf());
    dedupe_paths(paths)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut deduped = Vec::new();
    for path in paths {
        if !deduped.iter().any(|existing| existing == &path) {
            deduped.push(path);
        }
    }
    deduped
}

fn user_config_candidates() -> Vec<PathBuf> {
    let home = config::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let mut candidates = vec![home.join(".tmux.conf")];

    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        candidates.push(PathBuf::from(xdg).join("tmux").join("tmux.conf"));
    } else {
        candidates.push(home.join(".config").join("tmux").join("tmux.conf"));
    }

    candidates
}

fn forward_ctrl_f_bind_line() -> &'static str {
    "bind-key -n C-f send-keys C-f"
}

fn remove_legacy_ctrl_f_bindings(content: &str) -> String {
    let mut cleaned = Vec::new();
    let mut previous_was_legacy_comment = false;

    for line in content.lines() {
        if line.trim() == "# tmux-helper sessionizer" {
            previous_was_legacy_comment = true;
            continue;
        }

        if is_legacy_tmuxxer_ctrl_f_binding(line) {
            previous_was_legacy_comment = false;
            continue;
        }

        if previous_was_legacy_comment {
            cleaned.push("# tmux-helper sessionizer");
            previous_was_legacy_comment = false;
        }
        cleaned.push(line);
    }

    if previous_was_legacy_comment {
        cleaned.push("# tmux-helper sessionizer");
    }

    let mut output = cleaned.join("\n");
    if content.ends_with('\n') && !output.is_empty() {
        output.push('\n');
    }
    output
}

fn is_legacy_tmuxxer_ctrl_f_binding(line: &str) -> bool {
    let trimmed = line.trim_start();
    (trimmed.starts_with("bind-key -n C-f ") || trimmed.starts_with("bind -n C-f "))
        && trimmed.contains("tmuxxer")
}

pub fn has_ctrl_f_binding() -> io::Result<bool> {
    let path = active_config_path();
    if !path.exists() {
        return Ok(false);
    }
    Ok(find_block_span(&fs::read_to_string(path)?).is_some())
}

fn find_block_span(content: &str) -> Option<(usize, usize)> {
    let start = content.find(MARKER_START)?;
    let rest = &content[start..];
    let end_rel = rest.find(MARKER_END)? + MARKER_END.len();
    let end = start + end_rel;
    let end = if content[end..].starts_with('\n') {
        end + 1
    } else {
        end
    };
    Some((start, end))
}

#[cfg(test)]
mod tests;
