use super::*;
use crate::config::SearchRoot;
use std::path::PathBuf;

#[test]
fn format_list_includes_launch_settings_and_search_values() {
    let mut config = Config::default();
    config.sources.docker = false;
    config.docker.new_session = false;
    config.search.ignores = vec!["target".to_string(), ".git".to_string()];
    config.search.roots = vec![SearchRoot {
        path: PathBuf::from("/tmp/code"),
        depth: 2,
    }];

    let output = format_list(&config);

    assert!(output.contains("sources.sessions = true"));
    assert!(output.contains("sources.docker = false"));
    assert!(output.contains("docker.new_session = false"));
    assert!(output.contains("session.name_strategy = \"path\""));
    assert!(output.contains("search.ignore = [\"target\", \".git\"]"));
    assert!(output.contains("search.roots[0].path = \"/tmp/code\""));
    assert!(output.contains("search.roots[0].depth = 2"));
}
