use std::io::Write;
use std::process::{Command, Output, Stdio};

/// Run fzf with the given lines; returns the selected line or None if cancelled.
pub fn pick(items: &[String]) -> Option<String> {
    let output = run_fzf(items, &[])?;
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

    if let Some(mut stdin) = child.stdin.take() {
        for item in items {
            let _ = writeln!(stdin, "{item}");
        }
    }

    child.wait_with_output().ok()
}
