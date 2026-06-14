use super::*;

#[test]
fn disabled_docker_source_does_not_list_containers() {
    let mut config = config_with_sources(true, false, false);
    config.search.roots.clear();
    let provider = TestEntryProvider {
        sessions: Some(vec!["work".to_string()]),
        containers: None,
    };

    let (lines, _) = collect_entries_with(&config, &provider).unwrap();

    assert_eq!(lines, vec!["[session] work"]);
}

#[test]
fn disabled_session_source_does_not_list_tmux_sessions() {
    let config = config_with_sources(false, false, true);
    let provider = TestEntryProvider {
        sessions: None,
        containers: Some(vec![docker::Container {
            id: "c22bd1e7a321".to_string(),
            name: "web".to_string(),
            image: "nginx:alpine".to_string(),
        }]),
    };

    let (lines, _) = collect_entries_with(&config, &provider).unwrap();

    assert_eq!(lines, vec!["[docker] web — nginx:alpine (c22bd1e7a321)"]);
}

#[test]
fn enabled_sources_with_no_entries_return_clear_error() {
    let config = config_with_sources(true, false, false);
    let provider = TestEntryProvider {
        sessions: Some(Vec::new()),
        containers: None,
    };

    let err = collect_entries_with(&config, &provider).unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::NotFound);
    assert!(err.to_string().contains("no entries found"));
}

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

#[test]
fn ignore_relative_path_pattern_matches_at_any_depth() {
    let rules = ignore_rules(&["node_modules/*"]);
    let root = Path::new("/tmp/work");

    assert!(is_ignored(
        root,
        Path::new("/tmp/work/app/node_modules/typescript"),
        &rules
    ));
    assert!(is_ignored(
        root,
        Path::new("/tmp/work/node_modules/esbuild"),
        &rules
    ));
    assert!(!is_ignored(
        root,
        Path::new("/tmp/work/app/src/node_modulesx/typescript"),
        &rules
    ));
}

#[test]
fn ignore_dot_slash_relative_pattern_matches_at_any_depth() {
    let rules = ignore_rules(&["./folder/"]);
    let root = Path::new("/tmp/work");

    assert!(is_ignored(
        root,
        Path::new("/tmp/work/app/folder/sub"),
        &rules
    ));
    assert!(!is_ignored(root, Path::new("/tmp/work/app/src"), &rules));
}

#[test]
fn docker_session_names_are_prefixed_and_sanitized() {
    let container = docker::Container {
        id: "c22bd1e7a321".to_string(),
        name: "api.web/1".to_string(),
        image: "app:latest".to_string(),
    };

    assert_eq!(session_name_from_docker(&container), "docker_api_web_1");
}

#[test]
fn basename_session_name_strategy_preserves_legacy_names() {
    let name = session_name_from_dir(
        Path::new("/tmp/work/api.web"),
        SessionNameStrategy::Basename,
    );

    assert_eq!(name, "api_web");
}

#[test]
fn path_session_name_strategy_uses_readable_basename() {
    let name = session_name_from_dir(Path::new("/tmp/work/api.web"), SessionNameStrategy::Path);

    assert_eq!(name, "api_web");
}

#[test]
fn available_session_name_uses_base_when_free() {
    let existing = vec!["other".to_string()];

    assert_eq!(available_session_name("api", &existing), "api");
}

#[test]
fn available_session_name_adds_numeric_suffix_on_collision() {
    let existing = vec!["api".to_string(), "api-2".to_string(), "worker".to_string()];

    assert_eq!(available_session_name("api", &existing), "api-3");
}

fn ignore_rules(patterns: &[&str]) -> Vec<IgnoreRule> {
    patterns
        .iter()
        .map(|pattern| IgnoreRule::new(pattern))
        .collect()
}

fn config_with_sources(
    sessions: bool,
    directories: bool,
    docker_enabled: bool,
) -> crate::config::Config {
    let mut config = crate::config::Config::default();
    config.sources.sessions = sessions;
    config.sources.directories = directories;
    config.sources.docker = docker_enabled;
    config
}

struct TestEntryProvider {
    sessions: Option<Vec<String>>,
    containers: Option<Vec<docker::Container>>,
}

impl EntryProvider for TestEntryProvider {
    fn sessions(&self) -> Vec<String> {
        self.sessions
            .clone()
            .expect("tmux sessions should not be listed")
    }

    fn docker_containers(&self) -> Vec<docker::Container> {
        self.containers
            .clone()
            .expect("docker containers should not be listed")
    }
}
