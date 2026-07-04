use std::io;
use std::io::Write;
use std::process::{Command, Output, Stdio};

use crate::updates;

pub trait Picker {
    /// Run the picker with the given lines; returns the selected line or None if cancelled.
    fn pick(&self, items: &[String]) -> io::Result<Option<String>>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FzfPicker;

impl Picker for FzfPicker {
    fn pick(&self, items: &[String]) -> io::Result<Option<String>> {
        let header = updates::notice();
        let mut extra_args = Vec::new();
        if let Some(header) = header.as_deref() {
            extra_args.push("--header");
            extra_args.push(header);
        }

        let output = run_fzf(items, &extra_args)?;
        if !output.status.success() {
            return Ok(None);
        }
        let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selection.is_empty() {
            Ok(None)
        } else {
            Ok(Some(selection))
        }
    }
}

fn run_fzf(items: &[String], extra_args: &[&str]) -> io::Result<Output> {
    let mut cmd = Command::new("fzf");
    cmd.args(["--height=80%", "--layout=reverse", "--border"])
        .args(extra_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = cmd.spawn()?;
    updates::spawn_background_check_if_due();

    if let Some(mut stdin) = child.stdin.take() {
        // fzf may exit before reading all input (e.g. the user cancels
        // immediately); a broken pipe here is not a failure.
        if let Err(error) = write_items(&mut stdin, items) {
            if error.kind() != io::ErrorKind::BrokenPipe {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
    }

    child.wait_with_output()
}

fn write_items(stdin: &mut impl Write, items: &[String]) -> io::Result<()> {
    for item in items {
        writeln!(stdin, "{item}")?;
    }
    Ok(())
}
