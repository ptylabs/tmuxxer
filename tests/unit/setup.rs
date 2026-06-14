use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn append_ignore_rejects_duplicates() {
    let mut config = test_config(vec![PathBuf::from("/tmp")]);
    config.search.ignores = vec!["target".to_string()];

    assert!(matches!(
        append_ignore(&mut config, "target"),
        AppendResult::Duplicate
    ));
    assert!(matches!(
        append_ignore(&mut config, "node_modules"),
        AppendResult::Added
    ));
}

#[test]
fn toggle_ignore_adds_and_removes() {
    let home = PathBuf::from("/home/user");
    let mut config = test_config(vec![PathBuf::from("/tmp")]);
    config.search.ignores = vec!["target".to_string()];

    assert!(matches!(
        toggle_ignore(&mut config, &home, "node_modules"),
        ToggleResult::Added
    ));
    assert_eq!(config.search.ignores.len(), 2);

    assert!(matches!(
        toggle_ignore(&mut config, &home, "target"),
        ToggleResult::Removed(_)
    ));
    assert_eq!(config.search.ignores, vec!["node_modules".to_string()]);
}

#[test]
fn toggle_root_adds_and_removes() {
    let existing = PathBuf::from("/tmp/work");
    let extra = PathBuf::from("/tmp/other");
    let mut config = test_config(vec![existing.clone(), extra.clone()]);

    assert!(matches!(
        toggle_root(&mut config, existing.clone()).unwrap(),
        ToggleResult::Removed(_)
    ));
    assert_eq!(config.search.roots.len(), 1);
    assert_eq!(config.search.roots[0].path, extra);

    assert!(matches!(
        toggle_root(&mut config, PathBuf::from("/tmp/new")).unwrap(),
        ToggleResult::Added
    ));
    assert_eq!(config.search.roots.len(), 2);
}

#[test]
fn toggle_root_rejects_removing_only_root() {
    let path = PathBuf::from("/tmp/work");
    let mut config = test_config(vec![path.clone()]);

    let err = toggle_root(&mut config, path).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
}

#[test]
fn toggle_root_allows_removing_only_root_when_directories_disabled() {
    let path = PathBuf::from("/tmp/work");
    let mut config = test_config(vec![path.clone()]);
    config.sources.directories = false;

    assert!(matches!(
        toggle_root(&mut config, path).unwrap(),
        ToggleResult::Removed(_)
    ));
    assert!(config.search.roots.is_empty());
}

#[test]
#[cfg(unix)]
fn toggle_root_matches_canonical_duplicates() {
    let dir = unique_temp_dir("tmuxxer-setup-root");
    let alias = dir.join("alias");
    std::os::unix::fs::symlink(&dir, &alias).unwrap();

    let mut config = test_config(vec![dir.clone(), PathBuf::from("/tmp/other")]);

    assert!(matches!(
        toggle_root(&mut config, alias).unwrap(),
        ToggleResult::Removed(_)
    ));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn normalize_ignore_cli_input_expands_dot_to_stored_path() {
    let dir = unique_temp_dir("tmuxxer-setup");
    let previous = env::current_dir().ok();
    env::set_current_dir(&dir).unwrap();

    let stored = normalize_ignore_cli_input(&dir, ".").unwrap();

    assert_eq!(stored, config::stored_path(&dir));

    if let Some(previous) = previous {
        let _ = env::set_current_dir(previous);
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn normalize_ignore_cli_input_keeps_patterns() {
    let home = PathBuf::from("/home/user");
    assert_eq!(
        normalize_ignore_cli_input(&home, "target").unwrap(),
        "target"
    );
    assert_eq!(
        normalize_ignore_cli_input(&home, "./folder/").unwrap(),
        "folder"
    );
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

fn test_config(paths: Vec<PathBuf>) -> config::Config {
    let mut config = config::Config::default();
    config.search.roots = paths
        .into_iter()
        .map(|path| SearchRoot { path, depth: 1 })
        .collect();
    config
}
