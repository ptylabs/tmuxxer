use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config;
use crate::install;

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
    let tmuxxer = install::resolve_tmuxxer()?;
    let command = format!(
        "{} sessionize",
        install::shell_quote(&tmuxxer.to_string_lossy())
    );
    let bind_line = send_keys_bind_line(&command);
    let block = format!("{MARKER_START}\n{bind_line}\n{MARKER_END}\n");

    let mut content = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        String::new()
    };

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
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#{config_files}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .split(',')
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .rfind(|path| !path.starts_with("/etc") && path.is_file())
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

fn send_keys_bind_line(command: &str) -> String {
    format!(
        "bind-key -n C-f send-keys C-u \\; send-keys -l {} \\; send-keys Enter",
        install::tmux_double_quote(&command)
    )
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
mod tests {
    use super::*;

    #[test]
    fn tmux_binding_runs_picker_in_current_pane() {
        let line = send_keys_bind_line("tmuxxer sessionize");

        assert!(line.contains("send-keys C-u"));
        assert!(line.contains("send-keys -l \"tmuxxer sessionize\""));
        assert!(line.contains("send-keys Enter"));
        assert!(!line.contains("display-popup"));
        assert!(!line.contains("run-shell"));
    }
}
