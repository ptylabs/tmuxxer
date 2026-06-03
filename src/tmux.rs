use std::env;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn inside_tmux() -> bool {
    env::var_os("TMUX").is_some()
}

pub fn server_running() -> bool {
    Command::new("tmux")
        .args(["list-sessions"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn sessions() -> Vec<String> {
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output();

    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        _ => Vec::new(),
    }
}

pub fn has_session(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn new_session(name: &str, dir: &Path, detached: bool) -> io::Result<()> {
    let dir = dir.to_string_lossy();
    let mut cmd = Command::new("tmux");
    cmd.arg("new-session");
    if detached {
        cmd.arg("-d");
    }
    cmd.args(["-s", name, "-c", dir.as_ref()]);
    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "tmux new-session failed for '{name}'"
        )))
    }
}

pub fn switch_client(name: &str) -> io::Result<()> {
    let status = Command::new("tmux")
        .args(["switch-client", "-t", name])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "tmux switch-client failed for '{name}'"
        )))
    }
}

pub fn attach(name: &str) -> io::Result<()> {
    let status = Command::new("tmux").args(["attach", "-t", name]).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("tmux attach failed for '{name}'")))
    }
}
