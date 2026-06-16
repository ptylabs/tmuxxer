use super::*;
use std::cell::RefCell;

use proptest::prelude::*;

#[test]
fn disabled_docker_source_does_not_list_containers() {
    let mut config = config_with_sources(true, false, false);
    config.search.roots.clear();
    let tmux_client = TestTmux {
        sessions: Some(vec!["work".to_string()]),
        ..TestTmux::default()
    };
    let docker_client = TestDocker::unused();

    let (lines, _) = collect_entries_with(&config, &tmux_client, &docker_client).unwrap();

    assert_eq!(lines, vec!["[session] work"]);
}

#[test]
fn disabled_session_source_does_not_list_tmux_sessions() {
    let config = config_with_sources(false, false, true);
    let tmux_client = TestTmux::unused();
    let docker_client = TestDocker {
        containers: Some(vec![docker::Container {
            id: "c22bd1e7a321".to_string(),
            name: "web".to_string(),
            image: "nginx:alpine".to_string(),
        }]),
        ..TestDocker::default()
    };

    let (lines, _) = collect_entries_with(&config, &tmux_client, &docker_client).unwrap();

    assert_eq!(lines, vec!["[docker] web — nginx:alpine (c22bd1e7a321)"]);
}

#[test]
fn enabled_sources_with_no_entries_return_clear_error() {
    let config = config_with_sources(true, false, false);
    let tmux_client = TestTmux {
        sessions: Some(Vec::new()),
        ..TestTmux::default()
    };
    let docker_client = TestDocker::unused();

    let err = collect_entries_with(&config, &tmux_client, &docker_client).unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::NotFound);
    assert!(err.to_string().contains("no entries found"));
}

#[test]
fn run_with_uses_picker_trait_selection() {
    let config = config_with_sources(true, false, false);
    let tmux_client = TestTmux {
        sessions: Some(vec!["work".to_string()]),
        ..TestTmux::default()
    };
    let docker_client = TestDocker::unused();
    let picker = TestPicker {
        selection: Some("[session] work".to_string()),
    };

    run_with(&config, &tmux_client, &docker_client, &picker).unwrap();

    assert_eq!(tmux_client.calls.borrow().as_slice(), ["attach:work"]);
}

#[test]
fn run_with_can_exec_docker_through_trait() {
    let mut config = config_with_sources(false, false, true);
    config.docker.new_session = false;
    let tmux_client = TestTmux::unused();
    let docker_client = TestDocker {
        containers: Some(vec![docker::Container {
            id: "c22bd1e7a321".to_string(),
            name: "web".to_string(),
            image: "nginx:alpine".to_string(),
        }]),
        ..TestDocker::default()
    };
    let picker = TestPicker {
        selection: Some("[docker] web — nginx:alpine (c22bd1e7a321)".to_string()),
    };

    run_with(&config, &tmux_client, &docker_client, &picker).unwrap();

    assert_eq!(docker_client.calls.borrow().as_slice(), ["exec-shell:web"]);
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

proptest! {
    #[test]
    fn wildcard_literal_patterns_match_only_exact(
        pattern in "[a-z0-9._/-]{0,32}",
        text in "[a-z0-9._/-]{0,32}",
    ) {
        prop_assert_eq!(wildcard_match(&pattern, &text), pattern == text);
    }

    #[test]
    fn wildcard_star_spans_generated_middle(
        prefix in "[a-z0-9._-]{0,8}",
        middle in "[a-z0-9._-]{0,16}",
        suffix in "[a-z0-9._-]{0,8}",
    ) {
        let pattern = format!("{prefix}*{suffix}");
        let text = format!("{prefix}{middle}{suffix}");

        prop_assert!(wildcard_match(&pattern, &text));
    }

    #[test]
    fn component_ignore_matches_generated_component_at_any_depth(
        component in "[a-z][a-z0-9_-]{0,8}",
        before in prop::collection::vec("[a-z][a-z0-9_-]{0,8}", 0..4),
        after in prop::collection::vec("[a-z][a-z0-9_-]{0,8}", 0..4),
    ) {
        let root = Path::new("/tmp/work");
        let mut path = root.to_path_buf();
        for part in before {
            path.push(part);
        }
        path.push(&component);
        for part in after {
            path.push(part);
        }
        let rules = ignore_rules(&[&component]);

        prop_assert!(is_ignored(root, &path, &rules));
    }

    #[test]
    fn relative_path_wildcard_matches_generated_nested_paths(
        parent in "[a-z][a-z0-9_-]{0,8}",
        child in "[a-z][a-z0-9_-]{0,8}",
        before in prop::collection::vec("[a-z][a-z0-9_-]{0,8}", 0..4),
    ) {
        let root = Path::new("/tmp/work");
        let pattern = format!("{parent}/*");
        let rules = ignore_rules(&[&pattern]);

        let mut matched = root.to_path_buf();
        for part in &before {
            matched.push(part);
        }
        matched.push(&parent);
        matched.push(&child);

        let mut partial = root.to_path_buf();
        for part in before {
            partial.push(format!("Allowed{part}"));
        }
        partial.push(format!("{parent}x"));
        partial.push(child);

        prop_assert!(is_ignored(root, &matched, &rules));
        prop_assert!(!is_ignored(root, &partial, &rules));
    }
}

#[test]
fn sessionize_dir_uses_tmux_trait() {
    let tmux_client = TestTmux {
        inside_tmux: true,
        server_running: true,
        sessions: Some(vec!["api".to_string()]),
        ..TestTmux::default()
    };

    sessionize_dir(
        &tmux_client,
        Path::new("/tmp/work/api"),
        SessionNameStrategy::Path,
    )
    .unwrap();

    assert_eq!(
        tmux_client.calls.borrow().as_slice(),
        ["new-session:api-2:true", "switch-client:api-2"]
    );
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

#[derive(Default)]
struct TestTmux {
    sessions: Option<Vec<String>>,
    inside_tmux: bool,
    server_running: bool,
    calls: RefCell<Vec<String>>,
}

impl TestTmux {
    fn unused() -> Self {
        Self {
            sessions: None,
            ..Self::default()
        }
    }
}

impl tmux::TmuxCommand for TestTmux {
    fn inside_tmux(&self) -> bool {
        self.inside_tmux
    }

    fn server_running(&self) -> bool {
        self.server_running
    }

    fn sessions(&self) -> Vec<String> {
        self.sessions
            .clone()
            .expect("tmux sessions should not be listed")
    }

    fn has_session(&self, name: &str) -> bool {
        self.sessions
            .as_ref()
            .is_some_and(|sessions| sessions.iter().any(|session| session == name))
    }

    fn new_session(&self, name: &str, _dir: &Path, detached: bool) -> Result<(), tmux::TmuxError> {
        self.calls
            .borrow_mut()
            .push(format!("new-session:{name}:{detached}"));
        Ok(())
    }

    fn new_session_with_command(
        &self,
        name: &str,
        _command: &str,
        detached: bool,
    ) -> Result<(), tmux::TmuxError> {
        self.calls
            .borrow_mut()
            .push(format!("new-session-command:{name}:{detached}"));
        Ok(())
    }

    fn switch_client(&self, name: &str) -> Result<(), tmux::TmuxError> {
        self.calls
            .borrow_mut()
            .push(format!("switch-client:{name}"));
        Ok(())
    }

    fn attach(&self, name: &str) -> Result<(), tmux::TmuxError> {
        self.calls.borrow_mut().push(format!("attach:{name}"));
        Ok(())
    }
}

#[derive(Default)]
struct TestDocker {
    containers: Option<Vec<docker::Container>>,
    calls: RefCell<Vec<String>>,
}

impl TestDocker {
    fn unused() -> Self {
        Self {
            containers: None,
            ..Self::default()
        }
    }
}

impl docker::DockerCommand for TestDocker {
    fn containers(&self) -> Vec<docker::Container> {
        self.containers
            .clone()
            .expect("docker containers should not be listed")
    }

    fn shell_command(&self, container: &docker::Container) -> String {
        format!("docker exec -it {} sh", container.id)
    }

    fn exec_shell(&self, container: &docker::Container) -> Result<(), docker::DockerError> {
        self.calls
            .borrow_mut()
            .push(format!("exec-shell:{}", container.name));
        Ok(())
    }
}

struct TestPicker {
    selection: Option<String>,
}

impl fzf::Picker for TestPicker {
    fn pick(&self, _items: &[String]) -> Result<Option<String>, fzf::PickerError> {
        Ok(self.selection.clone())
    }
}
