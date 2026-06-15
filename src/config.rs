use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const BOOL_SETTING_KEYS: &[&str] = &[
    "sources.sessions",
    "sources.directories",
    "sources.docker",
    "docker.new_session",
    "updates.auto_check",
];
pub const STRING_SETTING_KEYS: &[&str] = &["session.name_strategy"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRoot {
    pub path: PathBuf,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub sources: SourceConfig,
    pub session: SessionConfig,
    pub docker: DockerConfig,
    pub updates: UpdateConfig,
    pub search: SearchConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedConfig {
    inner: Config,
}

impl ValidatedConfig {
    pub fn new(config: Config) -> Result<Self, ConfigError> {
        config.validate()?;
        Ok(Self { inner: config })
    }

    pub fn as_config(&self) -> &Config {
        &self.inner
    }

    pub fn into_inner(self) -> Config {
        self.inner
    }
}

impl Deref for ValidatedConfig {
    type Target = Config;

    fn deref(&self) -> &Self::Target {
        self.as_config()
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config not found at {path}")]
    NotFound { path: String },
    #[error("all picker sources are disabled")]
    AllSourcesDisabled,
    #[error("config has no search roots while sources.directories is true")]
    DirectoriesNeedRoots,
    #[error("unsupported config version {0}")]
    UnsupportedVersion(u8),
    #[error("invalid {key}: {value} (expected true or false)")]
    InvalidBool { key: String, value: String },
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl ConfigError {
    pub fn kind(&self) -> io::ErrorKind {
        match self {
            Self::NotFound { .. } => io::ErrorKind::NotFound,
            Self::Io(error) => error.kind(),
            Self::AllSourcesDisabled
            | Self::DirectoriesNeedRoots
            | Self::UnsupportedVersion(_)
            | Self::InvalidBool { .. }
            | Self::TomlParse(_)
            | Self::TomlSerialize(_) => io::ErrorKind::InvalidData,
        }
    }
}

impl From<ConfigError> for io::Error {
    fn from(error: ConfigError) -> Self {
        if let ConfigError::Io(error) = error {
            error
        } else {
            let kind = error.kind();
            io::Error::new(kind, error)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceConfig {
    pub sessions: bool,
    pub directories: bool,
    pub docker: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DockerConfig {
    pub new_session: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateConfig {
    pub auto_check: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionNameStrategy {
    Basename,
    #[default]
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionConfig {
    pub name_strategy: SessionNameStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SearchConfig {
    pub roots: Vec<SearchRoot>,
    pub ignores: Vec<String>,
}

impl Config {
    pub fn load() -> Result<ValidatedConfig, ConfigError> {
        let path = config_path();
        if !path.is_file() {
            return Err(ConfigError::NotFound {
                path: path.display().to_string(),
            });
        }
        parse_file(&path)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        save_to_path(&config_path(), self)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.sources.sessions && !self.sources.directories && !self.sources.docker {
            return Err(ConfigError::AllSourcesDisabled);
        }

        if self.sources.directories && self.search.roots.is_empty() {
            return Err(ConfigError::DirectoriesNeedRoots);
        }

        Ok(())
    }

    pub fn with_roots(roots: Vec<SearchRoot>) -> Self {
        let mut config = Self::default();
        config.sources.directories = !roots.is_empty();
        config.search.roots = roots;
        config
    }

    pub fn bool_setting(&self, key: &str) -> Option<bool> {
        match key {
            "sources.sessions" => Some(self.sources.sessions),
            "sources.directories" => Some(self.sources.directories),
            "sources.docker" => Some(self.sources.docker),
            "docker.new_session" => Some(self.docker.new_session),
            "updates.auto_check" => Some(self.updates.auto_check),
            _ => None,
        }
    }

    pub fn set_bool_setting(&mut self, key: &str, value: bool) -> bool {
        match key {
            "sources.sessions" => self.sources.sessions = value,
            "sources.directories" => self.sources.directories = value,
            "sources.docker" => self.sources.docker = value,
            "docker.new_session" => self.docker.new_session = value,
            "updates.auto_check" => self.updates.auto_check = value,
            _ => return false,
        }
        true
    }

    pub fn string_setting(&self, key: &str) -> Option<&'static str> {
        match key {
            "session.name_strategy" => Some(self.session.name_strategy.as_str()),
            _ => None,
        }
    }

    pub fn set_string_setting(&mut self, key: &str, value: &str) -> Result<bool, String> {
        match key {
            "session.name_strategy" => {
                self.session.name_strategy = SessionNameStrategy::parse(value)
                    .ok_or_else(|| "expected one of: basename, path".to_string())?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub fn toggle_bool_setting(&mut self, key: &str) -> Option<bool> {
        let value = !self.bool_setting(key)?;
        self.set_bool_setting(key, value);
        Some(value)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sources: SourceConfig {
                sessions: true,
                directories: false,
                docker: true,
            },
            session: SessionConfig::default(),
            docker: DockerConfig::default(),
            updates: UpdateConfig::default(),
            search: SearchConfig::default(),
        }
    }
}

impl SessionNameStrategy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Basename => "basename",
            Self::Path => "path",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "basename" => Some(Self::Basename),
            "path" => Some(Self::Path),
            _ => None,
        }
    }
}

impl Default for SourceConfig {
    fn default() -> Self {
        Self {
            sessions: true,
            directories: true,
            docker: true,
        }
    }
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self { new_session: true }
    }
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self { auto_check: true }
    }
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            name_strategy: SessionNameStrategy::Path,
        }
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("tmuxxer").join("config")
    } else {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("/"))
            .join(".config")
            .join("tmuxxer")
            .join("config")
    }
}

pub fn exists() -> bool {
    config_path().is_file()
}

pub fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

pub fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "y" | "1" | "on" => Some(true),
        "false" | "no" | "n" | "0" | "off" => Some(false),
        _ => None,
    }
}

fn save_to_path(path: &Path, config: &Config) -> Result<(), ConfigError> {
    config.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format_config(config)?)?;
    Ok(())
}

fn format_config(config: &Config) -> Result<String, ConfigError> {
    let roots = config
        .search
        .roots
        .iter()
        .map(|root| SearchRootTomlOut {
            path: path_for_config(&root.path),
            depth: root.depth.max(1),
        })
        .collect();

    let output = ConfigTomlOut {
        version: 2,
        sources: &config.sources,
        session: &config.session,
        docker: &config.docker,
        updates: &config.updates,
        search: SearchTomlOut {
            ignore: config.search.ignores.clone(),
            roots,
        },
    };

    let mut content = String::from("# Generated by tmuxxer setup\n\n");
    content.push_str(&toml::to_string_pretty(&output)?);
    Ok(content)
}

fn parse_file(path: &Path) -> Result<ValidatedConfig, ConfigError> {
    parse_content(&fs::read_to_string(path)?)
}

fn parse_content(content: &str) -> Result<ValidatedConfig, ConfigError> {
    let config = if looks_like_toml(content) {
        parse_toml_content(content)?
    } else {
        parse_legacy_content(content)?
    };
    ValidatedConfig::new(config)
}

fn looks_like_toml(content: &str) -> bool {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            return true;
        }
        if let Some((key, _)) = line.split_once('=') {
            if key.trim() == "version" {
                return true;
            }
        }
    }
    false
}

fn parse_toml_content(content: &str) -> Result<Config, ConfigError> {
    let raw: ConfigTomlIn = toml::from_str(content)?;

    let version = raw.version.unwrap_or(2);
    if version != 2 {
        return Err(ConfigError::UnsupportedVersion(version));
    }

    let sources = raw.sources.map(Into::into).unwrap_or_default();
    let session = raw.session.map(Into::into).unwrap_or_default();
    let docker = raw.docker.map(Into::into).unwrap_or_default();
    let updates = raw.updates.map(Into::into).unwrap_or_default();
    let search = raw.search.map(Into::into).unwrap_or_default();

    Ok(Config {
        sources,
        session,
        docker,
        updates,
        search,
    })
}

fn parse_legacy_content(content: &str) -> Result<Config, ConfigError> {
    let mut roots: Vec<SearchRoot> = Vec::new();
    let mut ignores = Vec::new();
    let mut docker_new_session = true;
    let mut default_depth = 1usize;
    let mut last_root = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value.trim();
        match key.as_str() {
            "path" => {
                roots.push(SearchRoot {
                    path: expand_tilde(value),
                    depth: default_depth,
                });
                last_root = Some(roots.len() - 1);
            }
            "depth" => {
                if let Ok(d) = value.parse::<usize>() {
                    let depth = d.max(1);
                    if let Some(index) = last_root {
                        roots[index].depth = depth;
                    } else {
                        default_depth = depth;
                    }
                }
            }
            "ignore" => push_unique_string(&mut ignores, value),
            "docker_new_session" => {
                docker_new_session = parse_bool(value).ok_or_else(|| ConfigError::InvalidBool {
                    key: key.clone(),
                    value: value.to_string(),
                })?;
            }
            _ => {}
        }
    }

    Ok(Config {
        sources: SourceConfig::default(),
        session: SessionConfig::default(),
        docker: DockerConfig {
            new_session: docker_new_session,
        },
        updates: UpdateConfig::default(),
        search: SearchConfig { roots, ignores },
    })
}

fn push_unique_string(values: &mut Vec<String>, value: &str) {
    if !value.is_empty() && !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

pub fn stored_path(path: &Path) -> String {
    path_for_config(path)
}

pub fn expand_path(home: &Path, input: &str) -> PathBuf {
    if input == "~" {
        return home.to_path_buf();
    }
    if let Some(rest) = input.strip_prefix("~/") {
        return home.join(rest);
    }
    PathBuf::from(input)
}

fn path_for_config(path: &Path) -> String {
    let home = home_dir().unwrap_or_else(|| PathBuf::from("/"));
    if path == home {
        return "~".to_string();
    }
    if let Ok(rest) = path.strip_prefix(&home) {
        let rest = rest.to_string_lossy();
        let rest = rest.strip_prefix('/').unwrap_or(&rest);
        return format!("~/{rest}");
    }
    path.display().to_string()
}

fn expand_tilde(path: &str) -> PathBuf {
    expand_path(&home_dir().unwrap_or_else(|| PathBuf::from("/")), path)
}

impl From<SourceConfigTomlIn> for SourceConfig {
    fn from(value: SourceConfigTomlIn) -> Self {
        Self {
            sessions: value.sessions.unwrap_or(true),
            directories: value.directories.unwrap_or(true),
            docker: value.docker.unwrap_or(true),
        }
    }
}

impl From<DockerConfigTomlIn> for DockerConfig {
    fn from(value: DockerConfigTomlIn) -> Self {
        Self {
            new_session: value.new_session.unwrap_or(true),
        }
    }
}

impl From<UpdateConfigTomlIn> for UpdateConfig {
    fn from(value: UpdateConfigTomlIn) -> Self {
        Self {
            auto_check: value.auto_check.unwrap_or(true),
        }
    }
}

impl From<SessionConfigTomlIn> for SessionConfig {
    fn from(value: SessionConfigTomlIn) -> Self {
        Self {
            name_strategy: value.name_strategy.unwrap_or_default(),
        }
    }
}

impl From<SearchConfigTomlIn> for SearchConfig {
    fn from(value: SearchConfigTomlIn) -> Self {
        let roots = value
            .roots
            .into_iter()
            .map(|root| SearchRoot {
                path: expand_tilde(&root.path),
                depth: root.depth.unwrap_or(1).max(1),
            })
            .collect();

        let mut ignores = Vec::new();
        for ignore in value.ignore {
            push_unique_string(&mut ignores, &ignore);
        }

        Self { roots, ignores }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigTomlIn {
    version: Option<u8>,
    sources: Option<SourceConfigTomlIn>,
    session: Option<SessionConfigTomlIn>,
    docker: Option<DockerConfigTomlIn>,
    updates: Option<UpdateConfigTomlIn>,
    search: Option<SearchConfigTomlIn>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceConfigTomlIn {
    sessions: Option<bool>,
    directories: Option<bool>,
    docker: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DockerConfigTomlIn {
    new_session: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateConfigTomlIn {
    auto_check: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionConfigTomlIn {
    name_strategy: Option<SessionNameStrategy>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchConfigTomlIn {
    #[serde(default)]
    ignore: Vec<String>,
    #[serde(default)]
    roots: Vec<SearchRootTomlIn>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchRootTomlIn {
    path: String,
    depth: Option<usize>,
}

#[derive(Serialize)]
struct ConfigTomlOut<'a> {
    version: u8,
    sources: &'a SourceConfig,
    session: &'a SessionConfig,
    docker: &'a DockerConfig,
    updates: &'a UpdateConfig,
    search: SearchTomlOut,
}

#[derive(Serialize)]
struct SearchTomlOut {
    ignore: Vec<String>,
    roots: Vec<SearchRootTomlOut>,
}

#[derive(Serialize)]
struct SearchRootTomlOut {
    path: String,
    depth: usize,
}

#[cfg(test)]
mod tests;
