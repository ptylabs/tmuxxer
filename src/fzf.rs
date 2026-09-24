use std::io;
use std::io::{BufWriter, Write};
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
        selection_from_output(output)
    }
}

fn selection_from_output(output: Output) -> io::Result<Option<String>> {
    match output.status.code() {
        Some(0) => {
            let mut bytes = output.stdout;
            if bytes.last() == Some(&0) {
                bytes.pop();
            }
            if bytes.is_empty() {
                return Ok(None);
            }
            String::from_utf8(bytes)
                .map(Some)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        }
        Some(1 | 130) => Ok(None),
        _ => Err(io::Error::other(format!("fzf failed ({})", output.status))),
    }
}

fn run_fzf(items: &[String], extra_args: &[&str]) -> io::Result<Output> {
    let mut cmd = Command::new("fzf");
    cmd.args([
        "--height=80%",
        "--layout=reverse",
        "--border",
        "--read0",
        "--print0",
        "--no-multi",
    ])
    .args(extra_args)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit());
    let mut child = cmd.spawn()?;
    updates::spawn_background_check_if_due();

    if let Some(stdin) = child.stdin.take() {
        let mut stdin = BufWriter::new(stdin);
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
        stdin.write_all(item.as_bytes())?;
        stdin.write_all(&[0])?;
    }
    stdin.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn writes_nul_delimited_items_and_flushes() {
        let mut output = Vec::new();
        {
            let mut writer = BufWriter::new(&mut output);
            write_items(&mut writer, &["space ".into(), "line\nbreak".into()]).unwrap();
            assert!(writer.buffer().is_empty());
        }
        assert_eq!(output, b"space \0line\nbreak\0");
    }

    #[test]
    #[cfg(unix)]
    fn selection_preserves_whitespace_and_reports_failures() {
        let output = |code, bytes: &[u8]| Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: bytes.to_vec(),
            stderr: Vec::new(),
        };
        assert_eq!(
            selection_from_output(output(0, b"line\nbreak \0")).unwrap(),
            Some("line\nbreak ".into())
        );
        for code in [1, 130] {
            assert_eq!(selection_from_output(output(code, b"")).unwrap(), None);
        }
        assert!(selection_from_output(output(2, b"")).is_err());
    }
}
