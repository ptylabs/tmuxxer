use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

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
    let ignore_rules: Vec<IgnoreRule> = config
        .ignores
        .iter()
        .map(|ignore| IgnoreRule::new(ignore))
        .collect();

    for name in tmux::sessions() {
        let display = format!("{SESSION_PREFIX}{name}");
        map.insert(display.clone(), Entry::Session(name));
        lines.push(display);
    }

    let mut dirs = Vec::new();
    for root in &config.roots {
        if root.path.is_dir() && !is_ignored(&root.path, &root.path, &ignore_rules) {
            collect_dirs(&root.path, &root.path, root.depth, &ignore_rules, &mut dirs);
        }
    }
    dirs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    for path in dirs {
        let label = path.file_name().and_then(OsStr::to_str).unwrap_or("?");
        let display = format!("{DIR_PREFIX}{label} — {}", path.display());
        map.insert(display.clone(), Entry::Dir(path));
        lines.push(display);
    }

    Ok((lines, map))
}

fn collect_dirs(
    search_root: &Path,
    dir: &Path,
    max_depth: usize,
    ignore_rules: &[IgnoreRule],
    out: &mut Vec<PathBuf>,
) {
    if max_depth == 0 {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && !is_ignored(search_root, &path, ignore_rules) {
            out.push(path.clone());
            collect_dirs(
                search_root,
                &path,
                max_depth.saturating_sub(1),
                ignore_rules,
                out,
            );
        }
    }
}

#[derive(Debug, Clone)]
struct IgnoreRule {
    raw: String,
    kind: IgnoreKind,
}

#[derive(Debug, Clone)]
enum IgnoreKind {
    Component,
    Path { pattern: PathBuf, absolute: bool },
}

impl IgnoreRule {
    fn new(raw: &str) -> Self {
        let raw = raw.trim().to_string();
        let is_path = raw.contains('/') || raw.starts_with('/') || raw.starts_with('~');
        let kind = if is_path {
            IgnoreKind::Path {
                absolute: raw.starts_with('/') || raw.starts_with('~'),
                pattern: expand_ignore_path(&raw),
            }
        } else {
            IgnoreKind::Component
        };
        Self { raw, kind }
    }

    fn matches(&self, root: &Path, path: &Path) -> bool {
        match &self.kind {
            IgnoreKind::Component => path.components().any(|component| {
                let Component::Normal(component) = component else {
                    return false;
                };
                let component = component.to_string_lossy();
                wildcard_match(&self.raw, &component)
            }),
            IgnoreKind::Path { pattern, absolute } => {
                if *absolute {
                    path_prefix_matches(path, pattern)
                } else {
                    path.strip_prefix(root)
                        .map(|relative| path_prefix_matches(relative, pattern))
                        .unwrap_or(false)
                }
            }
        }
    }
}

fn is_ignored(root: &Path, path: &Path, ignore_rules: &[IgnoreRule]) -> bool {
    ignore_rules.iter().any(|rule| rule.matches(root, path))
}

fn expand_ignore_path(raw: &str) -> PathBuf {
    if raw == "~" {
        return crate::config::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        return crate::config::home_dir()
            .unwrap_or_else(|| PathBuf::from("/"))
            .join(rest);
    }
    PathBuf::from(raw)
}

fn path_prefix_matches(path: &Path, pattern: &Path) -> bool {
    let pattern_s = pattern.to_string_lossy();
    if !pattern_s.contains('*') {
        return path.starts_with(pattern);
    }

    let mut current = Some(path);
    while let Some(candidate) = current {
        let candidate_s = candidate.to_string_lossy();
        if wildcard_match(&pattern_s, &candidate_s) {
            return true;
        }
        current = candidate.parent();
    }
    false
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let mut pattern_index = 0usize;
    let mut text_index = 0usize;
    let mut star_index = None;
    let mut star_text_index = 0usize;

    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == text[text_index] || pattern[pattern_index] == b'*')
        {
            if pattern[pattern_index] == b'*' {
                star_index = Some(pattern_index);
                star_text_index = text_index;
                pattern_index += 1;
            } else {
                pattern_index += 1;
                text_index += 1;
            }
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            star_text_index += 1;
            text_index = star_text_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
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
    let base = dir.file_name().and_then(OsStr::to_str).unwrap_or("session");
    base.replace('.', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_exact_component_matches_any_path_component() {
        let rules = ignore_rules(&["target"]);
        let root = Path::new("/tmp/work");

        assert!(is_ignored(root, Path::new("/tmp/work/app/target"), &rules));
        assert!(is_ignored(
            root,
            Path::new("/tmp/work/app/target/debug"),
            &rules
        ));
        assert!(!is_ignored(root, Path::new("/tmp/work/app/src"), &rules));
    }

    #[test]
    fn ignore_wildcard_component_matches_dot_directories() {
        let rules = ignore_rules(&[".*"]);
        let root = Path::new("/tmp/work");

        assert!(is_ignored(root, Path::new("/tmp/work/app/.git"), &rules));
        assert!(!is_ignored(root, Path::new("/tmp/work/app/src"), &rules));
    }

    #[test]
    fn ignore_tilde_path_prefix_matches_descendants() {
        let Some(home) = crate::config::home_dir() else {
            return;
        };
        let root = home.join("work");
        let ignored = home.join("work/tmp/project");
        let allowed = home.join("work/src/project");
        let rules = ignore_rules(&["~/work/tmp"]);

        assert!(is_ignored(&root, &ignored, &rules));
        assert!(!is_ignored(&root, &allowed, &rules));
    }

    fn ignore_rules(patterns: &[&str]) -> Vec<IgnoreRule> {
        patterns
            .iter()
            .map(|pattern| IgnoreRule::new(pattern))
            .collect()
    }
}
