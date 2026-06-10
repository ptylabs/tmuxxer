use std::env;
use std::io;
use std::path::PathBuf;
use std::process::Command;

pub fn resolve_tmuxxer() -> io::Result<PathBuf> {
    let output = Command::new("sh")
        .arg("-c")
        .arg("command -v tmuxxer")
        .output()?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    if let Ok(path) = env::current_exe() {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "tmuxxer")
        {
            return Ok(path);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "tmuxxer not found on PATH — install with: cargo install --path .",
    ))
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
mod tests {
    use super::*;

    #[test]
    fn shell_quote_handles_single_quotes() {
        assert_eq!(shell_quote("/tmp/it's/tmuxxer"), "'/tmp/it'\\''s/tmuxxer'");
    }
}
