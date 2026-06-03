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
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "tmuxxer not found on PATH — install with: cargo install --path .",
    ))
}
