use std::cmp::Ordering;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::num::ParseIntError;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use thiserror::Error;

const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/ptylabs/tmuxxer/releases/latest";
const RELEASES_URL: &str = "https://github.com/ptylabs/tmuxxer/releases";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct UpdateState {
    last_check_at: u64,
    latest_version: Option<String>,
    latest_url: Option<String>,
    dismissed_version: Option<String>,
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("release has no version")]
    ReleaseMissingVersion,
    #[error("update check needs curl or wget on PATH")]
    FetchUnavailable,
    #[error("update fetch command failed: {command}")]
    FetchCommandFailed { command: &'static str },
    #[error("invalid update state field {field}: {source}")]
    StateParse {
        field: &'static str,
        #[source]
        source: ParseIntError,
    },
    #[error("release JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl UpdateError {
    fn kind(&self) -> io::ErrorKind {
        match self {
            Self::FetchUnavailable => io::ErrorKind::NotFound,
            Self::ReleaseMissingVersion | Self::StateParse { .. } | Self::Json(_) => {
                io::ErrorKind::InvalidData
            }
            Self::FetchCommandFailed { .. } => io::ErrorKind::Other,
            Self::Io(error) => error.kind(),
        }
    }
}

impl From<UpdateError> for io::Error {
    fn from(error: UpdateError) -> Self {
        if let UpdateError::Io(error) = error {
            error
        } else {
            let kind = error.kind();
            io::Error::new(kind, error)
        }
    }
}

pub trait ReleaseFetcher {
    fn fetch(&self, url: &str) -> Result<String, UpdateError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CommandReleaseFetcher;

impl ReleaseFetcher for CommandReleaseFetcher {
    fn fetch(&self, url: &str) -> Result<String, UpdateError> {
        let mut attempted = false;

        if command_exists("curl") {
            attempted = true;
            let output = Command::new("curl")
                .args([
                    "--fail",
                    "--silent",
                    "--show-error",
                    "--location",
                    "--max-time",
                    "5",
                    "-H",
                    "User-Agent: tmuxxer-update-check",
                    url,
                ])
                .output()?;
            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).to_string());
            }
        }

        if command_exists("wget") {
            attempted = true;
            let output = Command::new("wget")
                .args([
                    "--quiet",
                    "--timeout=5",
                    "--user-agent=tmuxxer-update-check",
                    "-O",
                    "-",
                    url,
                ])
                .output()?;
            if output.status.success() {
                return Ok(String::from_utf8_lossy(&output.stdout).to_string());
            }
        }

        if attempted {
            Err(UpdateError::FetchCommandFailed {
                command: "curl/wget",
            })
        } else {
            Err(UpdateError::FetchUnavailable)
        }
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: Option<String>,
    name: Option<String>,
    html_url: Option<String>,
}

pub fn run(args: &[String]) -> io::Result<()> {
    match args {
        [cmd] if cmd == "--check" || cmd == "check" => run_manual_check(),
        [cmd] if cmd == "--dismiss" || cmd == "dismiss" => dismiss_available_update(),
        [] => print_update_hint(),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: tmuxxer update [--check|--dismiss]",
        )),
    }
}

pub fn notice() -> Option<String> {
    if updates_disabled() {
        return None;
    }

    let state = read_state().ok()?;
    let latest = state.latest_version.as_deref()?;
    if state.dismissed_version.as_deref() == Some(latest)
        || !version_is_newer(latest, current_version())
    {
        return None;
    }

    Some(format!(
        "tmuxxer {latest} available - run: tmuxxer update --check"
    ))
}

pub fn spawn_background_check_if_due() {
    if updates_disabled() || !check_is_due() {
        return;
    }

    let Ok(exe) = env::current_exe() else {
        return;
    };

    let _ = Command::new(exe)
        .arg("__tmuxxer_update_check")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

pub fn run_background_check() -> io::Result<()> {
    if updates_disabled() || !acquire_lock()? {
        return Ok(());
    }

    let result = match fetch_latest_release() {
        Ok((version, url)) => {
            let mut state = read_state().unwrap_or_default();
            state.last_check_at = now_secs();
            state.latest_version = Some(version);
            state.latest_url = Some(url);
            write_state(&state)
        }
        Err(_) => {
            let mut state = read_state().unwrap_or_default();
            state.last_check_at = now_secs();
            write_state(&state)
        }
    };

    let _ = fs::remove_file(lock_path());
    result
}

fn run_manual_check() -> io::Result<()> {
    let (version, url) = fetch_latest_release()?;
    let mut state = read_state().unwrap_or_default();
    state.last_check_at = now_secs();
    state.latest_version = Some(version.clone());
    state.latest_url = Some(url.clone());
    write_state(&state)?;

    if version_is_newer(&version, current_version()) {
        println!("tmuxxer {version} is available: {url}");
    } else {
        println!("tmuxxer is up to date ({})", current_version());
    }

    Ok(())
}

fn dismiss_available_update() -> io::Result<()> {
    let mut state = read_state().unwrap_or_default();
    let Some(version) = state.latest_version.clone() else {
        println!("No cached update to dismiss.");
        return Ok(());
    };

    state.dismissed_version = Some(version.clone());
    write_state(&state)?;
    println!("Dismissed tmuxxer {version}.");
    Ok(())
}

fn print_update_hint() -> io::Result<()> {
    if let Some(message) = notice() {
        println!("{message}");
    } else {
        println!("Run 'tmuxxer update --check' to check for updates.");
    }
    Ok(())
}

fn fetch_latest_release() -> Result<(String, String), UpdateError> {
    fetch_latest_release_with(&CommandReleaseFetcher)
}

fn fetch_latest_release_with(
    fetcher: &impl ReleaseFetcher,
) -> Result<(String, String), UpdateError> {
    let body = fetcher.fetch(LATEST_RELEASE_URL)?;
    parse_release(&body)
}

fn parse_release(body: &str) -> Result<(String, String), UpdateError> {
    let release: GitHubRelease = serde_json::from_str(body)?;
    let version = release
        .tag_name
        .or(release.name)
        .filter(|version| !version.trim().is_empty())
        .ok_or(UpdateError::ReleaseMissingVersion)?;
    let url = release
        .html_url
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| RELEASES_URL.to_string());
    Ok((normalize_version(&version).to_string(), url))
}

fn command_exists(command: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {command} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn check_is_due() -> bool {
    let state = read_state().unwrap_or_default();
    now_secs().saturating_sub(state.last_check_at) >= CHECK_INTERVAL_SECS
}

fn acquire_lock() -> io::Result<bool> {
    let path = lock_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            let _ = writeln!(file, "{}", now_secs());
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            let stale = fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|elapsed| elapsed.as_secs() > 10 * 60);
            if stale {
                let _ = fs::remove_file(&path);
                return acquire_lock();
            }
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

fn read_state() -> io::Result<UpdateState> {
    parse_state(&fs::read_to_string(state_path())?).map_err(Into::into)
}

fn write_state(state: &UpdateState) -> io::Result<()> {
    let path = state_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, format_state(state))
}

fn parse_state(content: &str) -> Result<UpdateState, UpdateError> {
    let mut state = UpdateState::default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "last_check_at" => {
                state.last_check_at =
                    value
                        .parse::<u64>()
                        .map_err(|source| UpdateError::StateParse {
                            field: "last_check_at",
                            source,
                        })?;
            }
            "latest_version" => state.latest_version = non_empty(value),
            "latest_url" => state.latest_url = non_empty(value),
            "dismissed_version" => state.dismissed_version = non_empty(value),
            _ => {}
        }
    }

    Ok(state)
}

fn format_state(state: &UpdateState) -> String {
    format!(
        "last_check_at = {}\nlatest_version = {}\nlatest_url = {}\ndismissed_version = {}\n",
        state.last_check_at,
        state.latest_version.as_deref().unwrap_or(""),
        state.latest_url.as_deref().unwrap_or(""),
        state.dismissed_version.as_deref().unwrap_or("")
    )
}

fn non_empty(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn version_is_newer(candidate: &str, current: &str) -> bool {
    let candidate = parse_version(candidate);
    let current = parse_version(current);

    for index in 0..candidate.numbers.len().max(current.numbers.len()) {
        let left = candidate.numbers.get(index).copied().unwrap_or(0);
        let right = current.numbers.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }

    match (
        candidate.pre_release.as_deref(),
        current.pre_release.as_deref(),
    ) {
        (None, Some(_)) => true,
        (Some(_), None) | (None, None) => false,
        (Some(left), Some(right)) => compare_pre_release(left, right).is_gt(),
    }
}

fn compare_pre_release(left: &str, right: &str) -> Ordering {
    let mut left_parts = left.split('.');
    let mut right_parts = right.split('.');

    loop {
        match (left_parts.next(), right_parts.next()) {
            (Some(left), Some(right)) => {
                let ordering = compare_pre_release_identifier(left, right);
                if !ordering.is_eq() {
                    return ordering;
                }
            }
            (Some(_), None) => return Ordering::Greater,
            (None, Some(_)) => return Ordering::Less,
            (None, None) => return Ordering::Equal,
        }
    }
}

fn compare_pre_release_identifier(left: &str, right: &str) -> Ordering {
    match (left.parse::<u64>(), right.parse::<u64>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => left.cmp(right),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedVersion {
    numbers: Vec<u64>,
    pre_release: Option<String>,
}

fn parse_version(value: &str) -> ParsedVersion {
    let value = normalize_version(value);
    let value = value.split_once('+').map(|(core, _)| core).unwrap_or(value);
    let (core, pre_release) = value
        .split_once('-')
        .map(|(core, pre)| (core, Some(pre.to_string())))
        .unwrap_or((value, None));
    let numbers = core
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect();

    ParsedVersion {
        numbers,
        pre_release,
    }
}

fn normalize_version(value: &str) -> &str {
    value.trim().trim_start_matches(['v', 'V'])
}

fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn updates_disabled() -> bool {
    env::var_os("TMUXXER_NO_UPDATE_CHECK").is_some()
}

fn state_path() -> PathBuf {
    state_dir().join("update-state")
}

fn lock_path() -> PathBuf {
    state_dir().join("update-check.lock")
}

fn state_dir() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_STATE_HOME") {
        PathBuf::from(xdg).join("tmuxxer")
    } else {
        crate::config::home_dir()
            .unwrap_or_else(|| PathBuf::from("/"))
            .join(".local")
            .join("state")
            .join("tmuxxer")
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
