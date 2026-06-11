use std::io::Write;
use std::process::{Command, Output, Stdio};

use crate::updates;

/// Run fzf with the given lines; returns the selected line or None if cancelled.
pub fn pick(items: &[String]) -> Option<String> {
    let header = updates::notice();
    let mut extra_args = Vec::new();
    if let Some(header) = header.as_deref() {
        extra_args.push("--header");
        extra_args.push(header);
    }

    let output = run_fzf(items, &extra_args)?;
    if !output.status.success() {
        return None;
    }
    let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if selection.is_empty() {
        None
    } else {
        Some(selection)
    }
}

fn run_fzf(items: &[String], extra_args: &[&str]) -> Option<Output> {
    let mut args = vec!["--height=80%", "--layout=reverse", "--border"];
    args.extend(extra_args);

    let mut cmd = Command::new("fzf");
    cmd.args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = cmd.spawn().ok()?;
    updates::spawn_background_check_if_due();

    if let Some(mut stdin) = child.stdin.take() {
        for item in items {
            let _ = writeln!(stdin, "{item}");
        }
    }

    child.wait_with_output().ok()
}
