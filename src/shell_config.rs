use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config;
use crate::install;

const MARKER_START: &str = "# >>> tmuxxer >>>";
const MARKER_END: &str = "# <<< tmuxxer <<<";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Nushell,
}

impl Shell {
    pub fn detect() -> Option<Self> {
        detect_parent_shell().or_else(|| {
            env::var("SHELL")
                .ok()
                .and_then(|path| Self::from_path(&path))
        })
    }

    pub fn from_path(path: &str) -> Option<Self> {
        let name = path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .trim()
            .trim_start_matches('-');

        match name {
            "bash" => Some(Self::Bash),
            "zsh" => Some(Self::Zsh),
            "fish" | "fsh" => Some(Self::Fish),
            "nu" | "nushell" => Some(Self::Nushell),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Bash => "Bash",
            Self::Zsh => "Zsh",
            Self::Fish => "Fish",
            Self::Nushell => "Nushell",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Bash => "bash",
            Self::Zsh => "zsh",
            Self::Fish => "fish",
            Self::Nushell => "nushell",
        }
    }

    pub fn rc_path(self) -> io::Result<PathBuf> {
        let home = config::home_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME not set"))?;

        Ok(match self {
            Self::Bash => home.join(".bashrc"),
            Self::Zsh => env::var_os("ZDOTDIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.clone())
                .join(".zshrc"),
            Self::Fish => xdg_config_dir(&home).join("fish").join("config.fish"),
            Self::Nushell => xdg_config_dir(&home).join("nushell").join("config.nu"),
        })
    }

    pub fn reload_hint(self) -> io::Result<String> {
        Ok(format!("source {}", display_user_path(&self.rc_path()?)))
    }
}

pub fn supported_shell_names() -> &'static str {
    "bash, zsh, fish, nushell"
}

pub fn install_ctrl_f_binding(shell: Shell) -> io::Result<PathBuf> {
    let path = shell.rc_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmuxxer = install::resolve_tmuxxer()?;
    let command = shell_command(shell, &tmuxxer);
    let block = binding_block(shell, &command);

    let mut content = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        String::from("# tmuxxer\n")
    };

    if let Some((start, end)) = find_block_span(&content) {
        content.replace_range(start..end, &block);
    } else if !content.ends_with('\n') {
        content.push('\n');
        content.push_str(&block);
    } else {
        content.push_str(&block);
    }

    fs::write(&path, content)?;
    Ok(path)
}

pub fn has_ctrl_f_binding(shell: Shell) -> io::Result<bool> {
    let path = shell.rc_path()?;
    if !path.exists() {
        return Ok(false);
    }
    Ok(find_block_span(&fs::read_to_string(path)?).is_some())
}

fn binding_block(shell: Shell, command: &str) -> String {
    match shell {
        Shell::Bash => format!(
            "{MARKER_START}\n\
if [[ $- == *i* ]]; then\n\
  _tmuxxer_sessionize() {{\n\
    {command}\n\
  }}\n\
  bind -x '\"\\C-f\": \"_tmuxxer_sessionize\"'\n\
fi\n\
{MARKER_END}\n"
        ),
        Shell::Zsh => format!(
            "{MARKER_START}\n\
if [[ -o interactive ]]; then\n\
  _tmuxxer_sessionize() {{\n\
    zle -I\n\
    {command}\n\
    zle reset-prompt\n\
  }}\n\
  zle -N _tmuxxer_sessionize\n\
  bindkey '^F' _tmuxxer_sessionize\n\
  bindkey -M emacs '^F' _tmuxxer_sessionize 2>/dev/null\n\
  bindkey -M viins '^F' _tmuxxer_sessionize 2>/dev/null\n\
  bindkey -M vicmd '^F' _tmuxxer_sessionize 2>/dev/null\n\
fi\n\
{MARKER_END}\n"
        ),
        Shell::Fish => format!(
            "{MARKER_START}\n\
if status is-interactive\n\
    function _tmuxxer_sessionize\n\
        {command}\n\
        commandline -f repaint\n\
    end\n\
    bind \\cf _tmuxxer_sessionize\n\
    bind -M insert \\cf _tmuxxer_sessionize\n\
end\n\
{MARKER_END}\n"
        ),
        Shell::Nushell => format!(
            "{MARKER_START}\n\
$env.config = ($env.config | upsert keybindings (\n\
    ($env.config | get --optional keybindings | default [] | where name != \"tmuxxer_sessionize\")\n\
    | append {{\n\
        name: \"tmuxxer_sessionize\"\n\
        modifier: control\n\
        keycode: char_f\n\
        mode: [emacs vi_insert vi_normal]\n\
        event: {{ send: executehostcommand cmd: {} }}\n\
    }}\n\
))\n\
{MARKER_END}\n",
            nu_quote(command)
        ),
    }
}

fn shell_command(shell: Shell, tmuxxer: &Path) -> String {
    let tmuxxer = tmuxxer.to_string_lossy();
    match shell {
        Shell::Bash | Shell::Zsh => {
            format!("{} sessionize", install::shell_quote(&tmuxxer))
        }
        Shell::Fish => format!("command {} sessionize", fish_quote(&tmuxxer)),
        Shell::Nushell => format!("run-external {} sessionize", nu_quote(&tmuxxer)),
    }
}

fn fish_quote(value: &str) -> String {
    let mut quoted = String::from("'");
    for ch in value.chars() {
        match ch {
            '\'' => quoted.push_str("\\'"),
            '\\' => quoted.push_str("\\\\"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('\'');
    quoted
}

fn nu_quote(value: &str) -> String {
    let mut quoted = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            _ => quoted.push(ch),
        }
    }
    quoted.push('"');
    quoted
}

fn xdg_config_dir(home: &Path) -> PathBuf {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
}

fn display_user_path(path: &Path) -> String {
    if let Some(home) = config::home_dir() {
        if let Ok(stripped) = path.strip_prefix(&home) {
            if stripped.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}

fn detect_parent_shell() -> Option<Shell> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let ppid = status.lines().find_map(|line| {
        line.strip_prefix("PPid:")
            .and_then(|value| value.trim().parse::<u32>().ok())
    })?;
    let command = fs::read_to_string(format!("/proc/{ppid}/comm")).ok()?;
    Shell::from_path(command.trim())
}

fn find_block_span(content: &str) -> Option<(usize, usize)> {
    let start = content.find(MARKER_START)?;
    let rest = &content[start..];
    let end_rel = rest.find(MARKER_END)? + MARKER_END.len();
    let end = start + end_rel;
    let end = if content[end..].starts_with('\n') {
        end + 1
    } else {
        end
    };
    Some((start, end))
}

#[cfg(test)]
mod tests;
