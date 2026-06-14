use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::config::{Config, SessionNameStrategy};
use crate::docker;
use crate::fzf;
use crate::tmux;

const SESSION_PREFIX: &str = "[session] ";
const DIR_PREFIX: &str = "[dir] ";
const DOCKER_PREFIX: &str = "[docker] ";

#[derive(Debug, Clone)]
enum Entry {
    Session(String),
    Dir(PathBuf),
    Docker(docker::Container),
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
        Entry::Dir(path) => sessionize_dir(path, config.session.name_strategy),
        Entry::Docker(container) => open_docker(container, config.docker.new_session),
    }
}

fn collect_entries(config: &Config) -> io::Result<(Vec<String>, HashMap<String, Entry>)> {
    collect_entries_with(config, &RealEntryProvider)
}

trait EntryProvider {
    fn sessions(&self) -> Vec<String>;
    fn docker_containers(&self) -> Vec<docker::Container>;
}

struct RealEntryProvider;

impl EntryProvider for RealEntryProvider {
    fn sessions(&self) -> Vec<String> {
        tmux::sessions()
    }

    fn docker_containers(&self) -> Vec<docker::Container> {
        docker::containers()
    }
}

fn collect_entries_with<P: EntryProvider>(
    config: &Config,
    provider: &P,
) -> io::Result<(Vec<String>, HashMap<String, Entry>)> {
    config.validate()?;

    let mut lines = Vec::new();
    let mut map = HashMap::new();
    let ignore_rules: Vec<IgnoreRule> = config
        .search
        .ignores
        .iter()
        .map(|ignore| IgnoreRule::new(ignore))
        .collect();

    if config.sources.sessions {
        for name in provider.sessions() {
            let display = format!("{SESSION_PREFIX}{name}");
            map.insert(display.clone(), Entry::Session(name));
            lines.push(display);
        }
    }

    if config.sources.docker {
        for container in provider.docker_containers() {
            let display = format!(
                "{DOCKER_PREFIX}{} — {} ({})",
                container.name, container.image, container.id
            );
            map.insert(display.clone(), Entry::Docker(container));
            lines.push(display);
        }
    }

    if config.sources.directories {
        let mut dirs = Vec::new();
        for root in &config.search.roots {
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
    }

    if lines.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no entries found for enabled picker sources",
        ));
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
    Path {
        pattern: PathBuf,
        absolute: bool,
        anchored_to_root: bool,
    },
}

impl IgnoreRule {
    fn new(raw: &str) -> Self {
        let raw = raw.trim().to_string();
        let is_path = raw.contains('/') || raw.starts_with('/') || raw.starts_with('~');
        let kind = if is_path {
            IgnoreKind::Path {
                absolute: raw.starts_with('/') || raw.starts_with('~'),
                anchored_to_root: raw.starts_with('/'),
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
            IgnoreKind::Path {
                pattern,
                absolute,
                anchored_to_root,
            } => {
                if *absolute {
                    path_prefix_matches(path, pattern)
                } else {
                    path.strip_prefix(root)
                        .map(|relative| relative_path_matches(relative, pattern, *anchored_to_root))
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
    let raw = raw.trim().trim_end_matches('/');
    let raw = raw.strip_prefix("./").unwrap_or(raw);
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
    let path_s = normalize_path(path);
    let pattern_s = normalize_path(pattern);
    if !pattern_s.contains('*') {
        return path_s == pattern_s
            || path_s
                .strip_prefix(&pattern_s)
                .is_some_and(|rest| rest.starts_with('/'));
    }

    for candidate in path_prefix_candidates(&path_s) {
        if wildcard_match(&pattern_s, candidate) {
            return true;
        }
    }
    false
}

fn relative_path_matches(path: &Path, pattern: &Path, anchored_to_root: bool) -> bool {
    if anchored_to_root {
        return path_prefix_matches(path, pattern);
    }

    let relative = normalize_path(path);
    for candidate in path_suffix_candidates(&relative) {
        if path_prefix_matches(Path::new(candidate), pattern) {
            return true;
        }
    }

    false
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn path_prefix_candidates(path: &str) -> Vec<&str> {
    let mut candidates = Vec::new();
    let mut end = path.len();
    loop {
        let candidate = &path[..end];
        if !candidate.is_empty() {
            candidates.push(candidate);
        }
        match candidate.rfind('/') {
            Some(index) => end = index,
            None => break,
        }
    }
    candidates
}

fn path_suffix_candidates(path: &str) -> Vec<&str> {
    let mut candidates = Vec::new();
    if !path.is_empty() {
        candidates.push(path);
    }
    for (index, ch) in path.char_indices() {
        if ch == '/' && index + 1 < path.len() {
            candidates.push(&path[index + 1..]);
        }
    }
    candidates
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

fn sessionize_dir(dir: &Path, name_strategy: SessionNameStrategy) -> io::Result<()> {
    let base_name = session_name_from_dir(dir, name_strategy);

    if !tmux::inside_tmux() && !tmux::server_running() {
        tmux::new_session(&base_name, dir, false)?;
        return Ok(());
    }

    let name = match name_strategy {
        SessionNameStrategy::Basename => base_name,
        SessionNameStrategy::Path => available_session_name(&base_name, &tmux::sessions()),
    };

    if !tmux::has_session(&name) {
        tmux::new_session(&name, dir, true)?;
    }

    if tmux::inside_tmux() {
        tmux::switch_client(&name)
    } else {
        tmux::attach(&name)
    }
}

fn open_docker(container: &docker::Container, new_session: bool) -> io::Result<()> {
    if new_session {
        sessionize_docker(container)
    } else {
        docker::exec_shell(container)
    }
}

fn sessionize_docker(container: &docker::Container) -> io::Result<()> {
    let name = session_name_from_docker(container);
    let command = docker::shell_command(container);

    if !tmux::inside_tmux() && !tmux::server_running() {
        tmux::new_session_with_command(&name, &command, false)?;
        return Ok(());
    }

    if !tmux::has_session(&name) {
        tmux::new_session_with_command(&name, &command, true)?;
    }

    if tmux::inside_tmux() {
        tmux::switch_client(&name)
    } else {
        tmux::attach(&name)
    }
}

fn session_name_from_dir(dir: &Path, name_strategy: SessionNameStrategy) -> String {
    let base = dir.file_name().and_then(OsStr::to_str).unwrap_or("session");
    let base = base.replace('.', "_");

    match name_strategy {
        SessionNameStrategy::Basename => base,
        SessionNameStrategy::Path => sanitize_session_name_part(&base),
    }
}

fn session_name_from_docker(container: &docker::Container) -> String {
    let name = container
        .name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();

    format!("docker_{name}")
}

fn available_session_name(base: &str, existing: &[String]) -> String {
    if !existing.iter().any(|name| name == base) {
        return base.to_string();
    }

    let mut index = 2usize;
    loop {
        let candidate = format!("{base}-{index}");
        if !existing.iter().any(|name| name == &candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn sanitize_session_name_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();

    if sanitized.chars().any(|ch| ch != '_') {
        sanitized
    } else {
        "session".to_string()
    }
}

#[cfg(test)]
#[path = "../tests/unit/sessionizer.rs"]
mod tests;
