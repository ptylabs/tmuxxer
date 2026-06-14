use std::env;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub fn resolve_tmuxxer() -> io::Result<PathBuf> {
    let output = Command::new("sh")
        .arg("-c")
        .arg("command -v tmuxxer")
        .output()?;
    let path_candidate = if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        }
    } else {
        None
    };

    resolve_tmuxxer_from(path_candidate, env::current_exe().ok())
}

fn resolve_tmuxxer_from(
    path_candidate: Option<PathBuf>,
    current_exe: Option<PathBuf>,
) -> io::Result<PathBuf> {
    if let Some(path) = path_candidate.and_then(durable_tmuxxer_path) {
        return Ok(path);
    }

    if let Some(path) = current_exe.and_then(durable_tmuxxer_path) {
        return Ok(path);
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "permanent shell integration needs an installed/durable tmuxxer; run: cargo install --path .",
    ))
}

fn durable_tmuxxer_path(path: PathBuf) -> Option<PathBuf> {
    if !is_tmuxxer_executable(&path) || is_cargo_target_executable(&path) {
        return None;
    }

    if let Ok(canonical) = path.canonicalize() {
        if !is_tmuxxer_executable(&canonical) || is_cargo_target_executable(&canonical) {
            return None;
        }
        if !path.is_absolute() {
            return Some(canonical);
        }
    }

    path.is_absolute().then_some(path)
}

fn is_tmuxxer_executable(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "tmuxxer")
}

fn is_cargo_target_executable(path: &Path) -> bool {
    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>();

    components.iter().enumerate().any(|(index, component)| {
        *component == "target"
            && components[index + 1..]
                .iter()
                .any(|component| matches!(*component, "debug" | "release" | "deps"))
    })
}

pub fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    let mut quoted = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

#[cfg(test)]
mod tests;
