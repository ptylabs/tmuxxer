use super::*;
use crate::test_support::TempDir;

#[test]
fn default_config_is_valid_without_directory_roots() {
    let config = Config::default();

    assert!(config.validate().is_ok());
    assert!(!config.sources.directories);
    assert!(ValidatedConfig::new(config).is_ok());
}

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
         [updates]\n\
         auto_check = false\n\n\
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
    assert!(!config.updates.auto_check);
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
    assert_eq!(config.updates, UpdateConfig::default());
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
    let message = err.to_string();
    assert!(message.contains("bad config: TOML parse error at line 2"));
    assert!(message.contains("unknown field `surprise`"));
    assert!(message.contains("tmuxxer init"));
    assert!(message.contains("tmuxxer user-config"));
}

#[test]
fn toml_parse_error_mentions_config_path_when_loaded_from_file() {
    let dir = TempDir::new("tmuxxer-bad-config");
    let path = dir.join("config");
    fs::write(&path, "version = 2\n\n[unknown]\nvalue = true\n").unwrap();

    let err = parse_file(&path).unwrap_err();

    let message = err.to_string();
    assert!(message.contains(&path.display().to_string()));
    assert!(message.contains("line 3"));
    assert!(message.contains("tmuxxer init"));
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
    assert!(config.updates.auto_check);
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
    let dir = TempDir::new("tmuxxer-config");
    let path = dir.join("config");
    let mut config = Config::with_roots(vec![SearchRoot {
        path: dir.join("work"),
        depth: 2,
    }]);
    config.search.ignores = vec!["target".to_string(), ".git".to_string()];
    config.docker.new_session = false;
    config.updates.auto_check = false;
    config.session.name_strategy = SessionNameStrategy::Basename;
    config.sources.sessions = false;

    save_to_path(&path, &config).unwrap();
    let loaded = parse_file(&path).unwrap();

    assert_eq!(loaded.as_config(), &config);
    assert!(fs::read_to_string(&path).unwrap().contains("version = 2"));
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
    Config::with_roots(vec![SearchRoot {
        path: PathBuf::from("/tmp/code"),
        depth: 1,
    }])
}
