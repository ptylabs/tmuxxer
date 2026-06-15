use std::env;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TmuxError {
    #[error("tmux {action} failed for '{target}'")]
    CommandFailed {
        action: &'static str,
        target: String,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<TmuxError> for io::Error {
    fn from(error: TmuxError) -> Self {
        if let TmuxError::Io(error) = error {
            error
        } else {
            io::Error::other(error)
        }
    }
}

pub trait TmuxCommand {
    fn inside_tmux(&self) -> bool;
    fn server_running(&self) -> bool;
    fn sessions(&self) -> Vec<String>;
    fn has_session(&self, name: &str) -> bool;
    fn new_session(&self, name: &str, dir: &Path, detached: bool) -> Result<(), TmuxError>;
    fn new_session_with_command(
        &self,
        name: &str,
        command: &str,
        detached: bool,
    ) -> Result<(), TmuxError>;
    fn switch_client(&self, name: &str) -> Result<(), TmuxError>;
    fn attach(&self, name: &str) -> Result<(), TmuxError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemTmux;

impl TmuxCommand for SystemTmux {
    fn inside_tmux(&self) -> bool {
        env::var_os("TMUX").is_some()
    }

    fn server_running(&self) -> bool {
        Command::new("tmux")
            .args(["list-sessions"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn sessions(&self) -> Vec<String> {
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

    fn has_session(&self, name: &str) -> bool {
        Command::new("tmux")
            .args(["has-session", "-t", name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn new_session(&self, name: &str, dir: &Path, detached: bool) -> Result<(), TmuxError> {
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
            Err(TmuxError::CommandFailed {
                action: "new-session",
                target: name.to_string(),
            })
        }
    }

    fn new_session_with_command(
        &self,
        name: &str,
        command: &str,
        detached: bool,
    ) -> Result<(), TmuxError> {
        let mut cmd = Command::new("tmux");
        cmd.arg("new-session");
        if detached {
            cmd.arg("-d");
        }
        cmd.args(["-s", name, command]);
        let status = cmd.status()?;
        if status.success() {
            Ok(())
        } else {
            Err(TmuxError::CommandFailed {
                action: "new-session",
                target: name.to_string(),
            })
        }
    }

    fn switch_client(&self, name: &str) -> Result<(), TmuxError> {
        let status = Command::new("tmux")
            .args(["switch-client", "-t", name])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(TmuxError::CommandFailed {
                action: "switch-client",
                target: name.to_string(),
            })
        }
    }

    fn attach(&self, name: &str) -> Result<(), TmuxError> {
        let status = Command::new("tmux").args(["attach", "-t", name]).status()?;
        if status.success() {
            Ok(())
        } else {
            Err(TmuxError::CommandFailed {
                action: "attach",
                target: name.to_string(),
            })
        }
    }
}

pub fn inside_tmux() -> bool {
    SystemTmux.inside_tmux()
}

pub fn server_running() -> bool {
    SystemTmux.server_running()
}

pub fn sessions() -> Vec<String> {
    SystemTmux.sessions()
}

pub fn has_session(name: &str) -> bool {
    SystemTmux.has_session(name)
}

pub fn new_session(name: &str, dir: &Path, detached: bool) -> io::Result<()> {
    SystemTmux
        .new_session(name, dir, detached)
        .map_err(Into::into)
}

pub fn new_session_with_command(name: &str, command: &str, detached: bool) -> io::Result<()> {
    SystemTmux
        .new_session_with_command(name, command, detached)
        .map_err(Into::into)
}

pub fn switch_client(name: &str) -> io::Result<()> {
    SystemTmux.switch_client(name).map_err(Into::into)
}

pub fn attach(name: &str) -> io::Result<()> {
    SystemTmux.attach(name).map_err(Into::into)
}
