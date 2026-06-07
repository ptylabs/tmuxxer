use std::env;
use std::io::{self, IsTerminal};

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const ACCENT: &str = "\x1b[38;5;203m";
const OK: &str = "\x1b[38;5;149m";
const WARN: &str = "\x1b[38;5;215m";

#[derive(Debug, Clone, Copy)]
pub struct TerminalUi {
    color: bool,
    width: usize,
}

impl TerminalUi {
    pub fn new() -> Self {
        let color = io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none();
        let width = env::var("COLUMNS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(80)
            .clamp(56, 92);

        Self { color, width }
    }

    pub fn banner(&self, title: &str, subtitle: &str, lines: &[impl AsRef<str>]) {
        println!("{}", self.paint(BOLD, title));
        println!("{}", self.paint(DIM, subtitle));
        self.panel(lines);
    }

    pub fn section(&self, title: &str, lines: &[impl AsRef<str>]) {
        println!();
        println!("{} {}", self.paint(ACCENT, "◇"), self.paint(ACCENT, title));
        self.panel(lines);
    }

    pub fn question(&self, label: &str, hint: &str) -> String {
        format!(
            "{} {} {} ",
            self.paint(ACCENT, "◆"),
            label,
            self.paint(DIM, hint)
        )
    }

    pub fn input(&self, label: &str) -> String {
        format!("{} {} ", self.paint(OK, "›"), label)
    }

    pub fn success(&self, message: &str) {
        println!("{} {message}", self.paint(OK, "✓"));
    }

    pub fn note(&self, message: &str) {
        println!("{} {message}", self.paint(DIM, "•"));
    }

    pub fn warn(&self, message: &str) {
        println!("{} {message}", self.paint(WARN, "!"));
    }

    fn panel(&self, lines: &[impl AsRef<str>]) {
        let inner_width = self.width.saturating_sub(4);
        println!("┌{}┐", "─".repeat(inner_width + 2));
        for line in lines {
            for wrapped in wrap(line.as_ref(), inner_width) {
                println!("│ {:inner_width$} │", wrapped);
            }
        }
        println!("└{}┘", "─".repeat(inner_width + 2));
    }

    fn paint(&self, style: &str, value: &str) -> String {
        if self.color {
            format!("{style}{value}{RESET}")
        } else {
            value.to_string()
        }
    }
}

fn wrap(line: &str, width: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current = String::new();

    for word in line.split_whitespace() {
        let separator = usize::from(!current.is_empty());
        if current.chars().count() + separator + word.chars().count() <= width {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
            continue;
        }

        if !current.is_empty() {
            lines.push(current);
            current = String::new();
        }

        if word.chars().count() <= width {
            current.push_str(word);
        } else {
            let mut chunk = String::new();
            for ch in word.chars() {
                if chunk.chars().count() == width {
                    lines.push(chunk);
                    chunk = String::new();
                }
                chunk.push(ch);
            }
            current = chunk;
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}
