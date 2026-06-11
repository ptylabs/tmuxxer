use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const BOOL_SETTING_KEYS: &[&str] = &[
    "sources.sessions",
    "sources.directories",
    "sources.docker",
    "docker.new_session",
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
    pub search: SearchConfig,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionNameStrategy {
    Basename,
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
    pub fn load() -> io::Result<Self> {
        let path = config_path();
        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("config not found at {}", path.display()),
            ));
        }
        parse_file(&path)
    }

    pub fn save(&self) -> io::Result<()> {
        save_to_path(&config_path(), self)
    }

    pub fn validate(&self) -> io::Result<()> {
        if !self.sources.sessions && !self.sources.directories && !self.sources.docker {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "all picker sources are disabled",
            ));
        }

        if self.sources.directories && self.search.roots.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "config has no search roots while sources.directories is true",
            ));
        }

        Ok(())
    }

    pub fn bool_setting(&self, key: &str) -> Option<bool> {
        match key {
            "sources.sessions" => Some(self.sources.sessions),
            "sources.directories" => Some(self.sources.directories),
            "sources.docker" => Some(self.sources.docker),
            "docker.new_session" => Some(self.docker.new_session),
            _ => None,
        }
    }

    pub fn set_bool_setting(&mut self, key: &str, value: bool) -> bool {
        match key {
            "sources.sessions" => self.sources.sessions = value,
            "sources.directories" => self.sources.directories = value,
            "sources.docker" => self.sources.docker = value,
            "docker.new_session" => self.docker.new_session = value,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            sources: SourceConfig::default(),
            session: SessionConfig::default(),
            docker: DockerConfig::default(),
            search: SearchConfig::default(),
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

fn save_to_path(path: &Path, config: &Config) -> io::Result<()> {
    config.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format_config(config)?)
}

fn format_config(config: &Config) -> io::Result<String> {
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
        search: SearchTomlOut {
            ignore: config.search.ignores.clone(),
            roots,
        },
    };

    let mut content = String::from("# Generated by tmuxxer setup\n\n");
    content.push_str(
        &toml::to_string_pretty(&output)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
    );
    Ok(content)
}

fn parse_file(path: &Path) -> io::Result<Config> {
    parse_content(&fs::read_to_string(path)?)
}

fn parse_content(content: &str) -> io::Result<Config> {
    let config = if looks_like_toml(content) {
        parse_toml_content(content)?
    } else {
        parse_legacy_content(content)?
    };
    config.validate()?;
    Ok(config)
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

fn parse_toml_content(content: &str) -> io::Result<Config> {
    let raw: ConfigTomlIn =
        toml::from_str(content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let version = raw.version.unwrap_or(2);
    if version != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported config version {version}"),
        ));
    }

    let sources = raw.sources.map(Into::into).unwrap_or_default();
    let session = raw.session.map(Into::into).unwrap_or_default();
    let docker = raw.docker.map(Into::into).unwrap_or_default();
    let search = raw.search.map(Into::into).unwrap_or_default();

    Ok(Config {
        sources,
        session,
        docker,
        search,
    })
}

fn parse_legacy_content(content: &str) -> io::Result<Config> {
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
                docker_new_session = parse_bool(value).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid {key}: {value} (expected true or false)"),
                    )
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

impl From<SessionConfigTomlIn> for SessionConfig {
    fn from(value: SessionConfigTomlIn) -> Self {
        Self {
            name_strategy: value.name_strategy.unwrap_or_default(),
        }
    }
}

impl Default for SessionNameStrategy {
    fn default() -> Self {
        Self::Path
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
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn toml_v2_parses_all_fields() {
        let config = parse_content(
            "version = 2\n\n\
             [sources]\n\
             sessions = false\n\
             directories = true\n\
             docker = false\n\n\
             [session]\n\
             name_strategy = \"basename\"\n\n\
             [docker]\n\
             new_session = false\n\n\
             [search]\n\
             ignore = [\"target\", \".git\"]\n\n\
             [[search.roots]]\n\
             path = \"/tmp/code\"\n\
             depth = 3\n",
        )
        .unwrap();

        assert!(!config.sources.sessions);
        assert!(config.sources.directories);
        assert!(!config.sources.docker);
        assert_eq!(config.session.name_strategy, SessionNameStrategy::Basename);
        assert!(!config.docker.new_session);
        assert_eq!(config.search.ignores, vec!["target", ".git"]);
        assert_eq!(config.search.roots[0].path, PathBuf::from("/tmp/code"));
        assert_eq!(config.search.roots[0].depth, 3);
    }

    #[test]
    fn toml_v2_defaults_missing_optional_sections() {
        let config = parse_content(
            "version = 2\n\n\
             [search]\n\n\
             [[search.roots]]\n\
             path = \"/tmp/code\"\n",
        )
        .unwrap();

        assert_eq!(config.sources, SourceConfig::default());
        assert_eq!(config.session, SessionConfig::default());
        assert_eq!(config.docker, DockerConfig::default());
        assert_eq!(config.search.ignores, Vec::<String>::new());
        assert_eq!(config.search.roots[0].depth, 1);
    }

    #[test]
    fn toml_v2_allows_no_roots_when_directories_disabled() {
        let config = parse_content(
            "version = 2\n\n\
             [sources]\n\
             sessions = true\n\
             directories = false\n\
             docker = false\n",
        )
        .unwrap();

        assert!(config.search.roots.is_empty());
        assert!(!config.sources.directories);
    }

    #[test]
    fn toml_v2_rejects_no_roots_when_directories_enabled() {
        let err = parse_content(
            "version = 2\n\n\
             [sources]\n\
             sessions = true\n\
             directories = true\n\
             docker = false\n",
        )
        .unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn toml_v2_rejects_all_sources_disabled() {
        let err = parse_content(
            "version = 2\n\n\
             [sources]\n\
             sessions = false\n\
             directories = false\n\
             docker = false\n",
        )
        .unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn toml_v2_rejects_unsupported_version() {
        let err = parse_content("version = 3\n").unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("unsupported config version 3"));
    }

    #[test]
    fn toml_v2_rejects_unknown_keys() {
        let err = parse_content(
            "version = 2\n\
             surprise = true\n\n\
             [search]\n\
             [[search.roots]]\n\
             path = \"/tmp/code\"\n",
        )
        .unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn toml_v2_rejects_unknown_session_name_strategy() {
        let err = parse_content(
            "version = 2\n\n\
             [session]\n\
             name_strategy = \"random\"\n\n\
             [search]\n\
             [[search.roots]]\n\
             path = \"/tmp/code\"\n",
        )
        .unwrap_err();

        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn legacy_global_depth_applies_to_all_paths() {
        let config = parse_content(
            "depth = 2\n\
             path = /tmp/code\n\
             path = /tmp/work\n",
        )
        .unwrap();

        assert_eq!(config.search.roots[0].depth, 2);
        assert_eq!(config.search.roots[1].depth, 2);
    }

    #[test]
    fn legacy_per_path_depth_parses_for_each_path() {
        let config = parse_content(
            "path = /tmp/code\n\
             depth = 1\n\n\
             path = /tmp/work\n\
             depth = 3\n",
        )
        .unwrap();

        assert_eq!(config.search.roots[0].path, PathBuf::from("/tmp/code"));
        assert_eq!(config.search.roots[0].depth, 1);
        assert_eq!(config.search.roots[1].path, PathBuf::from("/tmp/work"));
        assert_eq!(config.search.roots[1].depth, 3);
    }

    #[test]
    fn legacy_import_defaults_sources_to_enabled() {
        let config = parse_content("path = /tmp/code\n").unwrap();

        assert!(config.sources.sessions);
        assert!(config.sources.directories);
        assert!(config.sources.docker);
        assert_eq!(config.session, SessionConfig::default());
        assert!(config.docker.new_session);
    }

    #[test]
    fn legacy_docker_new_session_parses_false() {
        let config = parse_content(
            "docker_new_session = false\n\
             path = /tmp/code\n",
        )
        .unwrap();

        assert!(!config.docker.new_session);
    }

    #[test]
    fn ignores_round_trip_through_save_and_load() {
        let dir = unique_temp_dir("tmuxxer-config");
        let path = dir.join("config");
        let mut config = Config::default();
        config.search.roots = vec![SearchRoot {
            path: dir.join("work"),
            depth: 2,
        }];
        config.search.ignores = vec!["target".to_string(), ".git".to_string()];
        config.docker.new_session = false;
        config.session.name_strategy = SessionNameStrategy::Basename;
        config.sources.sessions = false;

        save_to_path(&path, &config).unwrap();
        let loaded = parse_file(&path).unwrap();

        assert_eq!(loaded, config);
        assert!(fs::read_to_string(&path).unwrap().contains("version = 2"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn bool_settings_get_set_and_toggle() {
        let mut config = sample_config();

        for key in BOOL_SETTING_KEYS {
            assert_eq!(config.bool_setting(key), Some(true));
            assert!(config.set_bool_setting(key, false));
            assert_eq!(config.bool_setting(key), Some(false));
            assert_eq!(config.toggle_bool_setting(key), Some(true));
            assert_eq!(config.bool_setting(key), Some(true));
        }

        assert_eq!(config.bool_setting("unknown"), None);
        assert!(!config.set_bool_setting("unknown", true));
        assert_eq!(config.toggle_bool_setting("unknown"), None);
    }

    #[test]
    fn string_settings_get_and_set_name_strategy() {
        let mut config = sample_config();

        assert_eq!(config.string_setting("session.name_strategy"), Some("path"));
        assert_eq!(
            config.set_string_setting("session.name_strategy", "basename"),
            Ok(true)
        );
        assert_eq!(config.session.name_strategy, SessionNameStrategy::Basename);
        assert_eq!(
            config.set_string_setting("session.name_strategy", "invalid"),
            Err("expected one of: basename, path".to_string())
        );
        assert_eq!(config.set_string_setting("unknown", "path"), Ok(false));
        assert_eq!(config.string_setting("unknown"), None);
    }

    fn sample_config() -> Config {
        let mut config = Config::default();
        config.search.roots = vec![SearchRoot {
            path: PathBuf::from("/tmp/code"),
            depth: 1,
        }];
        config
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
