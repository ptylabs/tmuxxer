use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

use crate::config;
use crate::install;

const MARKER_START: &str = "# >>> tmuxxer >>>";
const MARKER_END: &str = "# <<< tmuxxer <<<";

/// Config file tmux loads (same order as tmux: TMUX_CONF, then XDG, then ~/.tmux.conf).
pub fn active_config_path() -> PathBuf {
    if let Ok(path) = env::var("TMUX_CONF") {
        return PathBuf::from(path);
    }
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        let path = PathBuf::from(xdg).join("tmux").join("tmux.conf");
        if path.exists() {
            return path;
        }
    }
    let xdg_default = config::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".config")
        .join("tmux")
        .join("tmux.conf");
    if xdg_default.exists() {
        return xdg_default;
    }
    config::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".tmux.conf")
}

pub fn install_ctrl_f_binding() -> io::Result<()> {
    let path = active_config_path();
    let tmuxxer = install::resolve_tmuxxer()?;

    let bind_line = format!(
        "bind-key -n C-f run-shell -b \"{}\"",
        tmuxxer.display()
    );
    let block = format!("{MARKER_START}\n{bind_line}\n{MARKER_END}\n");

    let mut content = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        String::new()
    };

    if let Some((start, end)) = find_block_span(&content) {
        content.replace_range(start..end, &block);
    } else if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
        content.push_str(&block);
    } else {
        content.push_str(&block);
    }

    fs::write(&path, content)?;
    Ok(())
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
