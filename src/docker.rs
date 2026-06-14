use std::collections::HashSet;
use std::io;
use std::process::{Command, Stdio};
use thiserror::Error;

use crate::install;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
}

#[derive(Debug, Error)]
pub enum DockerError {
    #[error("docker {action} failed for '{container}'")]
    CommandFailed {
        action: &'static str,
        container: String,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<DockerError> for io::Error {
    fn from(error: DockerError) -> Self {
        if let DockerError::Io(error) = error {
            error
        } else {
            io::Error::other(error)
        }
    }
}

pub trait DockerCommand {
    fn containers(&self) -> Vec<Container>;
    fn shell_command(&self, container: &Container) -> String;
    fn exec_shell(&self, container: &Container) -> Result<(), DockerError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemDocker;

impl DockerCommand for SystemDocker {
    fn containers(&self) -> Vec<Container> {
        let output = Command::new("docker")
            .args([
                "ps",
                "--filter",
                "status=running",
                "--format",
                "{{.ID}}\t{{.Names}}\t{{.Image}}",
            ])
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let mut containers = parse_containers(&String::from_utf8_lossy(&output.stdout));
                containers.sort_by(|left, right| left.name.cmp(&right.name));
                containers
            }
            _ => Vec::new(),
        }
    }

    fn shell_command(&self, container: &Container) -> String {
        let shell = self.shell_for(container);
        shell_command_with_shell(container, &shell)
    }

    fn exec_shell(&self, container: &Container) -> Result<(), DockerError> {
        let shell = self.shell_for(container);
        let status = Command::new("docker")
            .args(["exec", "-it", &container.id, &shell])
            .status()?;

        if status.success() {
            Ok(())
        } else {
            Err(DockerError::CommandFailed {
                action: "exec",
                container: container.name.clone(),
            })
        }
    }
}

impl SystemDocker {
    fn shell_for(&self, container: &Container) -> String {
        self.detect_shell(&container.id)
            .unwrap_or_else(|| "sh".to_string())
    }

    fn detect_shell(&self, container_id: &str) -> Option<String> {
        let configured_shell = self.configured_shell(container_id);
        shell_candidates(configured_shell.as_deref())
            .into_iter()
            .find(|shell| self.shell_runs(container_id, shell))
    }

    fn configured_shell(&self, container_id: &str) -> Option<String> {
        let output = Command::new("docker")
            .args([
                "inspect",
                "--format",
                "{{range .Config.Env}}{{println .}}{{end}}",
                container_id,
            ])
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }

        parse_shell_env(&String::from_utf8_lossy(&output.stdout))
    }

    fn shell_runs(&self, container_id: &str, shell: &str) -> bool {
        Command::new("docker")
            .args(["exec", container_id, shell, "-c", "exit 0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

pub fn containers() -> Vec<Container> {
    SystemDocker.containers()
}

pub fn shell_command(container: &Container) -> String {
    SystemDocker.shell_command(container)
}

pub fn exec_shell(container: &Container) -> io::Result<()> {
    SystemDocker.exec_shell(container).map_err(Into::into)
}

fn shell_command_with_shell(container: &Container, shell: &str) -> String {
    format!(
        "docker exec -it {} {}",
        install::shell_quote(&container.id),
        install::shell_quote(shell)
    )
}

fn parse_containers(output: &str) -> Vec<Container> {
    output.lines().filter_map(parse_container_line).collect()
}

fn parse_container_line(line: &str) -> Option<Container> {
    let mut parts = line.splitn(3, '\t');
    let id = parts.next()?.trim();
    let name = parts.next()?.trim();
    let image = parts.next()?.trim();

    if id.is_empty() || name.is_empty() {
        return None;
    }

    Some(Container {
        id: id.to_string(),
        name: name.to_string(),
        image: if image.is_empty() {
            "unknown".to_string()
        } else {
            image.to_string()
        },
    })
}

fn parse_shell_env(env_lines: &str) -> Option<String> {
    env_lines.lines().find_map(|line| {
        let shell = line.strip_prefix("SHELL=")?.trim();
        if is_supported_shell(shell) {
            Some(shell.to_string())
        } else {
            None
        }
    })
}

fn shell_candidates(configured_shell: Option<&str>) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    if let Some(shell) = configured_shell {
        push_unique_shell(shell, &mut candidates, &mut seen);
    }

    for shell in [
        "bash",
        "/bin/bash",
        "/usr/bin/bash",
        "zsh",
        "/bin/zsh",
        "/usr/bin/zsh",
        "fish",
        "/bin/fish",
        "/usr/bin/fish",
        "sh",
        "/bin/sh",
        "/usr/bin/sh",
        "ash",
        "/bin/ash",
        "/usr/bin/ash",
        "dash",
        "/bin/dash",
        "/usr/bin/dash",
        "ksh",
        "/bin/ksh",
        "/usr/bin/ksh",
    ] {
        push_unique_shell(shell, &mut candidates, &mut seen);
    }

    candidates
}

fn push_unique_shell(shell: &str, candidates: &mut Vec<String>, seen: &mut HashSet<String>) {
    if is_supported_shell(shell) && seen.insert(shell.to_string()) {
        candidates.push(shell.to_string());
    }
}

fn is_supported_shell(shell: &str) -> bool {
    matches!(
        shell.rsplit('/').next().unwrap_or(shell),
        "bash" | "zsh" | "fish" | "sh" | "ash" | "dash" | "ksh"
    )
}

#[cfg(test)]
mod tests;
