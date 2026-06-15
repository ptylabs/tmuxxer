use std::io;
use std::io::Write;
use std::process::{Command, Output, Stdio};
use thiserror::Error;

use crate::updates;

/// Run fzf with the given lines; returns the selected line or None if cancelled.
pub fn pick(items: &[String]) -> io::Result<Option<String>> {
    FzfPicker.pick(items).map_err(Into::into)
}

#[derive(Debug, Error)]
pub enum PickerError {
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<PickerError> for io::Error {
    fn from(error: PickerError) -> Self {
        match error {
            PickerError::Io(error) => error,
        }
    }
}

pub trait Picker {
    fn pick(&self, items: &[String]) -> Result<Option<String>, PickerError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FzfPicker;

impl Picker for FzfPicker {
    fn pick(&self, items: &[String]) -> Result<Option<String>, PickerError> {
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

fn run_fzf(items: &[String], extra_args: &[&str]) -> Result<Output, PickerError> {
    let mut args = vec!["--height=80%", "--layout=reverse", "--border"];
    args.extend(extra_args);

    let mut cmd = Command::new("fzf");
    cmd.args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = cmd.spawn()?;
    updates::spawn_background_check_if_due();

    if let Some(mut stdin) = child.stdin.take() {
        for item in items {
            writeln!(stdin, "{item}")?;
        }
    }

    Ok(child.wait_with_output()?)
}
