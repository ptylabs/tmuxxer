use std::borrow::Cow;
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
    run_with(
        config.as_config(),
        &tmux::SystemTmux,
        &docker::SystemDocker,
        &fzf::FzfPicker,
    )
}

fn run_with<T, D, P>(
    config: &Config,
    tmux_client: &T,
    docker_client: &D,
    picker: &P,
) -> io::Result<()>
where
    T: tmux::TmuxCommand + Sync,
    D: docker::DockerCommand + Sync,
    P: fzf::Picker,
{
    let (lines, entries) = collect_entries_with(config, tmux_client, docker_client)?;

    let Some(selection) = picker.pick(&lines)? else {
        return Ok(());
    };

    let entry = lines
        .iter()
        .position(|line| line == &selection)
        .map(|index| &entries[index])
        .ok_or_else(|| io::Error::other("invalid selection"))?;

    match entry {
        Entry::Session(name) => attach_session(tmux_client, name),
        Entry::Dir(path) => sessionize_dir(tmux_client, path, config.session.name_strategy),
        Entry::Docker(container) => open_docker(
            tmux_client,
            docker_client,
            container,
            config.docker.new_session,
        ),
    }
}

fn collect_entries_with<T, D>(
    config: &Config,
    tmux_client: &T,
    docker_client: &D,
) -> io::Result<(Vec<String>, Vec<Entry>)>
where
    T: tmux::TmuxCommand + Sync,
    D: docker::DockerCommand + Sync,
{
    config.validate()?;

    let (sessions, containers, dirs) = std::thread::scope(|scope| {
        let sessions = config
            .sources
            .sessions
            .then(|| scope.spawn(|| tmux_client.sessions()));
        let containers = config
            .sources
            .docker
            .then(|| scope.spawn(|| docker_client.containers()));
        let dirs = collect_directories(config);
        let sessions = sessions
            .map(|worker| worker.join())
            .transpose()
            .map_err(|_| io::Error::other("tmux listing worker failed"))?
            .unwrap_or_default();
        let containers = containers
            .map(|worker| worker.join())
            .transpose()
            .map_err(|_| io::Error::other("Docker listing worker failed"))?
            .unwrap_or_default();
        Ok::<_, io::Error>((sessions, containers, dirs))
    })?;

    let capacity = sessions.len() + containers.len() + dirs.len();
    let mut lines = Vec::with_capacity(capacity);
    let mut entries = Vec::with_capacity(capacity);
    for name in sessions {
        lines.push(format!("{SESSION_PREFIX}{name}"));
        entries.push(Entry::Session(name));
    }
    for container in containers {
        lines.push(format!(
            "{DOCKER_PREFIX}{} — {} ({})",
            container.name, container.image, container.id
        ));
        entries.push(Entry::Docker(container));
    }
    for path in dirs {
        let label = path.file_name().and_then(OsStr::to_str).unwrap_or("?");
        lines.push(format!("{DIR_PREFIX}{label} — {}", path.display()));
        entries.push(Entry::Dir(path));
    }

    if lines.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no entries found for enabled picker sources",
        ));
    }

    Ok((lines, entries))
}

fn collect_directories(config: &Config) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if !config.sources.directories {
        return dirs;
    }
    let rules: Vec<_> = config
        .search
        .ignores
        .iter()
        .map(|rule| IgnoreRule::new(rule))
        .collect();
    for root in &config.search.roots {
        if root.path.is_dir() && !is_ignored(&root.path, &root.path, &rules) {
            collect_dirs(&root.path, root.depth, &rules, &mut dirs);
        }
    }
    dirs.sort_unstable_by(|a, b| a.file_name().cmp(&b.file_name()).then_with(|| a.cmp(b)));
    dirs.dedup();
    dirs
}

fn collect_dirs(root: &Path, max_depth: usize, rules: &[IgnoreRule], out: &mut Vec<PathBuf>) {
    let mut pending = vec![(root.to_path_buf(), max_depth)];
    while let Some((dir, depth)) = pending.pop() {
        if depth == 0 {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() && !file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if is_ignored(root, &path, rules) || (file_type.is_symlink() && !path.is_dir()) {
                continue;
            }
            if depth > 1 && !(file_type.is_symlink() && links_to_ancestor(&path)) {
                pending.push((path.clone(), depth - 1));
            }
            out.push(path);
        }
    }
}

fn links_to_ancestor(path: &Path) -> bool {
    let Ok(target) = path.canonicalize() else {
        return true;
    };
    path.ancestors().skip(1).any(|ancestor| {
        ancestor
            .canonicalize()
            .is_ok_and(|ancestor| ancestor == target)
    })
}

#[derive(Debug, Clone)]
struct IgnoreRule {
    raw: String,
    kind: IgnoreKind,
}

#[derive(Debug, Clone)]
enum IgnoreKind {
    Component,
    Path { pattern: String, absolute: bool },
}

impl IgnoreRule {
    fn new(raw: &str) -> Self {
        let raw = raw.trim().to_string();
        let is_path = raw.contains('/') || raw.starts_with('~');
        let kind = if is_path {
            IgnoreKind::Path {
                absolute: raw.starts_with('/') || raw.starts_with('~'),
                pattern: normalize_path(&expand_ignore_path(&raw)).into_owned(),
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
                    path_prefix_matches(&normalize_path(path), pattern)
                } else {
                    path.strip_prefix(root)
                        .map(|relative| relative_path_matches(&normalize_path(relative), pattern))
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

fn path_prefix_matches(path: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return path == pattern
            || path
                .strip_prefix(pattern)
                .is_some_and(|rest| rest.starts_with('/'));
    }
    let mut candidate = path;
    while !candidate.is_empty() {
        if wildcard_match(pattern, candidate) {
            return true;
        }
        let Some(index) = candidate.rfind('/') else {
            break;
        };
        candidate = &candidate[..index];
    }
    false
}

fn relative_path_matches(path: &str, pattern: &str) -> bool {
    let mut candidate = path;
    while !candidate.is_empty() {
        if path_prefix_matches(candidate, pattern) {
            return true;
        }
        let Some((_, rest)) = candidate.split_once('/') else {
            break;
        };
        candidate = rest;
    }
    false
}

fn normalize_path(path: &Path) -> Cow<'_, str> {
    let path = path.to_string_lossy();
    if path.contains('\\') {
        Cow::Owned(path.replace('\\', "/"))
    } else {
        path
    }
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

fn attach_session<T: tmux::TmuxCommand>(tmux_client: &T, name: &str) -> io::Result<()> {
    if tmux_client.inside_tmux() {
        tmux_client.switch_client(name)?;
    } else {
        tmux_client.attach(name)?;
    }
    Ok(())
}

fn sessionize_dir<T: tmux::TmuxCommand>(
    tmux_client: &T,
    dir: &Path,
    name_strategy: SessionNameStrategy,
) -> io::Result<()> {
    let base_name = session_name_from_dir(dir, name_strategy);

    if !tmux_client.inside_tmux() && !tmux_client.server_running() {
        tmux_client.new_session(&base_name, dir, false)?;
        return Ok(());
    }

    let name = match name_strategy {
        SessionNameStrategy::Basename => base_name,
        SessionNameStrategy::Path => available_session_name(&base_name, &tmux_client.sessions()),
    };

    if !tmux_client.has_session(&name) {
        tmux_client.new_session(&name, dir, true)?;
    }

    attach_session(tmux_client, &name)
}

fn open_docker<T, D>(
    tmux_client: &T,
    docker_client: &D,
    container: &docker::Container,
    new_session: bool,
) -> io::Result<()>
where
    T: tmux::TmuxCommand,
    D: docker::DockerCommand,
{
    if new_session {
        sessionize_docker(tmux_client, docker_client, container)
    } else {
        docker_client.exec_shell(container)?;
        Ok(())
    }
}

fn sessionize_docker<T, D>(
    tmux_client: &T,
    docker_client: &D,
    container: &docker::Container,
) -> io::Result<()>
where
    T: tmux::TmuxCommand,
    D: docker::DockerCommand,
{
    let name = session_name_from_docker(container);
    let command = docker_client.shell_command(container);

    if !tmux_client.inside_tmux() && !tmux_client.server_running() {
        tmux_client.new_session_with_command(&name, &command, false)?;
        return Ok(());
    }

    if !tmux_client.has_session(&name) {
        tmux_client.new_session_with_command(&name, &command, true)?;
    }

    attach_session(tmux_client, &name)
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
    format!("docker_{}", sanitize_session_chars(&container.name))
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

fn sanitize_session_chars(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn sanitize_session_name_part(value: &str) -> String {
    let sanitized = sanitize_session_chars(value);
    if sanitized.chars().any(|ch| ch != '_') {
        sanitized
    } else {
        "session".to_string()
    }
}

#[cfg(test)]
mod tests;
