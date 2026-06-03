use std::process::{Command, Stdio};

pub fn ensure_tools() -> Result<(), String> {
    if !runs_ok("tmux", &["-V"]) {
        return Err("tmuxxer: tmux not found on PATH (install tmux)".into());
    }
    if !runs_ok("fzf", &["--version"]) {
        return Err("tmuxxer: fzf not found on PATH (install fzf)".into());
    }
    Ok(())
}

fn runs_ok(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
