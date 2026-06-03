use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::fzf;
use crate::tmux;

const SESSION_PREFIX: &str = "[session] ";
const DIR_PREFIX: &str = "[dir] ";

#[derive(Debug, Clone)]
enum Entry {
    Session(String),
    Dir(PathBuf),
}

pub fn run() -> io::Result<()> {
    let config = Config::load()?;
    let (lines, map) = collect_entries(&config)?;

    let Some(selection) = fzf::pick(&lines) else {
        return Ok(());
    };

    let entry = map
        .get(&selection)
        .ok_or_else(|| io::Error::other("invalid selection"))?;

    match entry {
        Entry::Session(name) => attach_session(name),
        Entry::Dir(path) => sessionize_dir(path),
    }
}

fn collect_entries(config: &Config) -> io::Result<(Vec<String>, HashMap<String, Entry>)> {
    let mut lines = Vec::new();
    let mut map = HashMap::new();

    for name in tmux::sessions() {
        let display = format!("{SESSION_PREFIX}{name}");
        map.insert(display.clone(), Entry::Session(name));
        lines.push(display);
    }

    let mut dirs = Vec::new();
    for root in &config.roots {
        if root.is_dir() {
            collect_dirs(root, config.depth, &mut dirs);
        }
    }
    dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    for path in dirs {
        let label = path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("?");
        let display = format!("{DIR_PREFIX}{label} — {}", path.display());
        map.insert(display.clone(), Entry::Dir(path));
        lines.push(display);
    }

    Ok((lines, map))
}

fn collect_dirs(root: &Path, max_depth: usize, out: &mut Vec<PathBuf>) {
    if max_depth == 0 {
        return;
    }
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.push(path.clone());
            collect_dirs(&path, max_depth.saturating_sub(1), out);
        }
    }
}

fn attach_session(name: &str) -> io::Result<()> {
    if tmux::inside_tmux() {
        tmux::switch_client(name)
    } else {
        tmux::attach(name)
    }
}

fn sessionize_dir(dir: &Path) -> io::Result<()> {
    let name = session_name_from_dir(dir);

    if !tmux::inside_tmux() && !tmux::server_running() {
        tmux::new_session(&name, dir, false)?;
        return Ok(());
    }

    if !tmux::has_session(&name) {
        tmux::new_session(&name, dir, true)?;
    }

    if tmux::inside_tmux() {
        tmux::switch_client(&name)
    } else {
        tmux::attach(&name)
    }
}

fn session_name_from_dir(dir: &Path) -> String {
    let base = dir
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("session");
    base.replace('.', "_")
}
