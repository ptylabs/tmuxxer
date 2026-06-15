use std::cmp::Ordering;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
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
    auto_check_disabled: bool,
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
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: Option<String>,
    browser_download_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseAsset {
    name: String,
    url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LatestRelease {
    version: String,
    url: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InstallMethod {
    Cargo,
    Script(PathBuf),
}

pub fn run(args: &[String]) -> io::Result<()> {
    match args {
        [] => run_update(),
        [cmd] if cmd == "--disable-auto" || cmd == "disable-auto" => disable_auto_updates(),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: tmuxxer update [--disable-auto]",
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
        "tmuxxer {latest} is available. Update with 'tmuxxer update' or disable automatic checks with 'tmuxxer update --disable-auto' or 'tmuxxer config set updates.auto_check false'."
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

fn run_update() -> io::Result<()> {
    let release = fetch_latest_release_details()?;
    let mut state = read_state().unwrap_or_default();
    state.last_check_at = now_secs();
    state.latest_version = Some(release.version.clone());
    state.latest_url = Some(release.url.clone());
    write_state(&state)?;

    if !version_is_newer(&release.version, current_version()) {
        println!("tmuxxer is up to date ({})", current_version());
        return Ok(());
    }

    println!(
        "Updating tmuxxer {} -> {}",
        current_version(),
        release.version
    );

    match detect_install_method()? {
        InstallMethod::Cargo => update_with_cargo(),
        InstallMethod::Script(exe) => update_script_binary(&release, &exe),
    }
}

fn disable_auto_updates() -> io::Result<()> {
    let mut state = read_state().unwrap_or_default();
    state.auto_check_disabled = true;
    write_state(&state)?;

    if let Ok(mut config) = crate::config::Config::load().map(|config| config.into_inner()) {
        config.updates.auto_check = false;
        config.save()?;
        println!("Automatic update checks disabled in config.");
    } else {
        println!("Automatic update checks disabled.");
    }

    Ok(())
}

fn detect_install_method() -> io::Result<InstallMethod> {
    let exe = crate::install::resolve_tmuxxer().or_else(|_| {
        env::current_exe().and_then(|path| {
            if path.is_absolute() {
                Ok(path)
            } else {
                path.canonicalize()
            }
        })
    })?;

    if is_cargo_install_path(&exe) {
        return Ok(InstallMethod::Cargo);
    }

    Ok(InstallMethod::Script(exe))
}

fn is_cargo_install_path(path: &Path) -> bool {
    let mut paths = vec![path.to_path_buf()];
    if let Ok(canonical) = path.canonicalize() {
        if canonical != path {
            paths.push(canonical);
        }
    }

    paths.iter().any(|path| {
        matches_cargo_home(path)
            || cargo_root_from_bin(path).is_some_and(|root| {
                root.join(".crates.toml").is_file() || root.join(".crates2.json").is_file()
            })
    })
}

fn matches_cargo_home(path: &Path) -> bool {
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| crate::config::home_dir().map(|home| home.join(".cargo")));
    let Some(cargo_home) = cargo_home else {
        return false;
    };

    path == cargo_home.join("bin").join("tmuxxer")
}

fn cargo_root_from_bin(path: &Path) -> Option<PathBuf> {
    if path.file_name().and_then(|name| name.to_str()) != Some("tmuxxer") {
        return None;
    }

    let bin = path.parent()?;
    if bin.file_name().and_then(|name| name.to_str()) != Some("bin") {
        return None;
    }

    bin.parent().map(Path::to_path_buf)
}

fn update_with_cargo() -> io::Result<()> {
    if !command_exists("cargo") {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "tmuxxer was installed with cargo, but cargo is not on PATH",
        ));
    }

    let status = Command::new("cargo")
        .args(["install", "tmuxxer"])
        .status()?;
    if status.success() {
        println!("Updated tmuxxer with cargo.");
        Ok(())
    } else {
        Err(io::Error::other(
            "cargo install tmuxxer failed while updating tmuxxer",
        ))
    }
}

fn update_script_binary(release: &LatestRelease, exe: &Path) -> io::Result<()> {
    let target = release_target().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "automatic binary updates are only supported for Linux x86_64, aarch64, and armv7",
        )
    })?;
    let asset = find_release_asset(release, target).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "release {} has no asset for {target}; update cannot continue from {}",
                release.version, release.url
            ),
        )
    })?;

    let temp_dir = update_temp_dir(&release.version)?;
    fs::create_dir_all(&temp_dir)?;
    let archive = temp_dir.join(&asset.name);

    download_to_file(&asset.url, &archive)?;
    if let Some(checksum_asset) = find_checksum_asset(release) {
        let checksums = temp_dir.join(&checksum_asset.name);
        download_to_file(&checksum_asset.url, &checksums)?;
        verify_archive_checksum(&checksums, &asset.name, &archive)?;
    } else {
        eprintln!(
            "Warning: release {} has no checksum asset; continuing without verification.",
            release.version
        );
    }
    extract_archive(&archive, &temp_dir)?;
    let extracted = find_extracted_tmuxxer(&temp_dir)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "release asset {} did not contain a tmuxxer binary",
                asset.name
            ),
        )
    })?;

    replace_executable(&extracted, exe)?;
    let _ = fs::remove_dir_all(&temp_dir);
    println!("Updated tmuxxer to {}.", release.version);
    Ok(())
}

fn find_release_asset<'a>(release: &'a LatestRelease, target: &str) -> Option<&'a ReleaseAsset> {
    let version = release.version.trim_start_matches('v');
    let names = [
        format!("tmuxxer-{version}-{target}.tar.gz"),
        format!("tmuxxer-v{version}-{target}.tar.gz"),
    ];

    release
        .assets
        .iter()
        .find(|asset| names.contains(&asset.name))
}

fn find_checksum_asset(release: &LatestRelease) -> Option<&ReleaseAsset> {
    let version = release.version.trim_start_matches('v');
    let names = [
        format!("tmuxxer-{version}-sha256sums.txt"),
        format!("tmuxxer-v{version}-sha256sums.txt"),
        "SHA256SUMS".to_string(),
        "sha256sums.txt".to_string(),
    ];

    release
        .assets
        .iter()
        .find(|asset| names.contains(&asset.name))
}

fn release_target() -> Option<&'static str> {
    match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("linux", "arm") => Some("armv7-unknown-linux-gnueabihf"),
        _ => None,
    }
}

fn download_to_file(url: &str, path: &Path) -> io::Result<()> {
    if command_exists("curl") {
        let status = Command::new("curl")
            .args([
                "--fail",
                "--silent",
                "--show-error",
                "--location",
                "--max-time",
                "60",
                "-H",
                "User-Agent: tmuxxer-update",
                "-o",
                &path.display().to_string(),
                url,
            ])
            .status()?;
        if status.success() {
            return Ok(());
        }
    }

    if command_exists("wget") {
        let status = Command::new("wget")
            .args([
                "--quiet",
                "--timeout=60",
                "--user-agent=tmuxxer-update",
                "-O",
                &path.display().to_string(),
                url,
            ])
            .status()?;
        if status.success() {
            return Ok(());
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "tmuxxer update needs curl or wget on PATH",
    ))
}

fn verify_archive_checksum(checksums: &Path, asset_name: &str, archive: &Path) -> io::Result<()> {
    let content = fs::read_to_string(checksums)?;
    let expected = checksum_for_asset(&content, asset_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} does not contain a SHA256 entry for {asset_name}",
                checksums.display()
            ),
        )
    })?;

    if !is_sha256_hex(expected) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid SHA256 digest for {asset_name}"),
        ));
    }

    let actual = sha256_file(archive)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("checksum mismatch for {asset_name}"),
        ))
    }
}

fn checksum_for_asset<'a>(content: &'a str, asset_name: &str) -> Option<&'a str> {
    content.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let hash = fields.next()?;
        let name = fields
            .next()?
            .trim_start_matches('*')
            .trim_start_matches("./");

        (name == asset_name).then_some(hash)
    })
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn sha256_file(path: &Path) -> io::Result<String> {
    if command_exists("sha256sum") {
        let output = Command::new("sha256sum").arg(path).output()?;
        return first_output_field("sha256sum", output);
    }

    if command_exists("shasum") {
        let output = Command::new("shasum")
            .args(["-a", "256"])
            .arg(path)
            .output()?;
        return first_output_field("shasum", output);
    }

    if command_exists("openssl") {
        let output = Command::new("openssl")
            .args(["dgst", "-sha256", "-r"])
            .arg(path)
            .output()?;
        return first_output_field("openssl", output);
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "checksum verification needs sha256sum, shasum, or openssl on PATH",
    ))
}

fn first_output_field(command: &'static str, output: Output) -> io::Result<String> {
    if !output.status.success() {
        return Err(io::Error::other(format!("{command} failed")));
    }

    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{command} output was empty"),
            )
        })
}

fn extract_archive(archive: &Path, dir: &Path) -> io::Result<()> {
    if !command_exists("tar") {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "tmuxxer update needs tar on PATH to unpack release assets",
        ));
    }

    validate_archive_entries(archive)?;
    let status = Command::new("tar")
        .args([
            "-xzf",
            &archive.display().to_string(),
            "-C",
            &dir.display().to_string(),
        ])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to unpack {}", archive.display()),
        ))
    }
}

fn validate_archive_entries(archive: &Path) -> io::Result<()> {
    let output = Command::new("tar")
        .args(["-tzf", &archive.display().to_string()])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to list {}", archive.display()),
        ));
    }

    for entry in String::from_utf8_lossy(&output.stdout).lines() {
        if archive_entry_is_unsafe(entry) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("release asset contains unsafe path {entry:?}"),
            ));
        }
    }

    Ok(())
}

fn archive_entry_is_unsafe(entry: &str) -> bool {
    entry.is_empty()
        || entry.starts_with('/')
        || entry == ".."
        || entry.starts_with("../")
        || entry.ends_with("/..")
        || entry.contains("/../")
}
fn find_extracted_tmuxxer(dir: &Path) -> io::Result<Option<PathBuf>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_extracted_tmuxxer(&path)? {
                return Ok(Some(found));
            }
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "tmuxxer")
        {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn replace_executable(source: &Path, destination: &Path) -> io::Result<()> {
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("cannot update {}", destination.display()),
        )
    })?;
    let temp = parent.join(format!(".tmuxxer-update-{}", std::process::id()));

    fs::copy(source, &temp).map_err(|error| {
        if error.kind() == io::ErrorKind::PermissionDenied {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "cannot write {}; rerun with sufficient permissions",
                    parent.display()
                ),
            )
        } else {
            error
        }
    })?;
    make_executable(&temp)?;
    fs::rename(&temp, destination).map_err(|error| {
        let _ = fs::remove_file(&temp);
        if error.kind() == io::ErrorKind::PermissionDenied {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "cannot replace {}; rerun with sufficient permissions",
                    destination.display()
                ),
            )
        } else {
            error
        }
    })
}

#[cfg(unix)]
fn make_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn fetch_latest_release() -> Result<(String, String), UpdateError> {
    fetch_latest_release_with(&CommandReleaseFetcher)
}

fn fetch_latest_release_details() -> Result<LatestRelease, UpdateError> {
    fetch_latest_release_details_with(&CommandReleaseFetcher)
}

fn fetch_latest_release_with(
    fetcher: &impl ReleaseFetcher,
) -> Result<(String, String), UpdateError> {
    fetch_latest_release_details_with(fetcher).map(|release| (release.version, release.url))
}

fn fetch_latest_release_details_with(
    fetcher: &impl ReleaseFetcher,
) -> Result<LatestRelease, UpdateError> {
    let body = fetcher.fetch(LATEST_RELEASE_URL)?;
    parse_release_details(&body)
}

fn parse_release_details(body: &str) -> Result<LatestRelease, UpdateError> {
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
    let assets = release
        .assets
        .into_iter()
        .filter_map(|asset| {
            Some(ReleaseAsset {
                name: asset.name?.trim().to_string(),
                url: asset.browser_download_url?.trim().to_string(),
            })
        })
        .filter(|asset| !asset.name.is_empty() && !asset.url.is_empty())
        .collect();

    Ok(LatestRelease {
        version: normalize_version(&version).to_string(),
        url,
        assets,
    })
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
            "auto_check_disabled" => {
                state.auto_check_disabled = crate::config::parse_bool(value).unwrap_or(false);
            }
            _ => {}
        }
    }

    Ok(state)
}

fn format_state(state: &UpdateState) -> String {
    format!(
        "last_check_at = {}\nlatest_version = {}\nlatest_url = {}\ndismissed_version = {}\nauto_check_disabled = {}\n",
        state.last_check_at,
        state.latest_version.as_deref().unwrap_or(""),
        state.latest_url.as_deref().unwrap_or(""),
        state.dismissed_version.as_deref().unwrap_or(""),
        state.auto_check_disabled
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
    if env::var_os("TMUXXER_NO_UPDATE_CHECK").is_some() {
        return true;
    }

    if let Ok(config) = crate::config::Config::load() {
        return !config.updates.auto_check;
    }

    read_state()
        .map(|state| state.auto_check_disabled)
        .unwrap_or(false)
}

fn state_path() -> PathBuf {
    state_dir().join("update-state")
}

fn lock_path() -> PathBuf {
    state_dir().join("update-check.lock")
}

fn update_temp_dir(version: &str) -> io::Result<PathBuf> {
    Ok(state_dir().join(format!(
        "update-{}-{}",
        normalize_version(version),
        now_secs()
    )))
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
