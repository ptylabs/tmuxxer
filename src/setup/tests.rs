use super::*;
use crate::terminal_ui::TerminalUi;
use crate::test_support::{CurrentDirGuard, TempDir};

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
    let dir = TempDir::new("tmuxxer-setup-root");
    let alias = dir.join("alias");
    std::os::unix::fs::symlink(dir.path(), &alias).unwrap();

    let mut config = test_config(vec![dir.path().to_path_buf(), PathBuf::from("/tmp/other")]);

    assert!(matches!(
        toggle_root(&mut config, alias).unwrap(),
        ToggleResult::Removed(_)
    ));
}

#[test]
fn normalize_ignore_cli_input_expands_dot_to_stored_path() {
    let dir = TempDir::new("tmuxxer-setup");
    let _cwd = CurrentDirGuard::change_to(dir.path());

    let stored = normalize_ignore_cli_input(dir.path(), ".").unwrap();

    assert_eq!(stored, config::stored_path(dir.path()));
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

#[test]
fn optional_user_config_setup_warning_keeps_init_success() {
    let ui = TerminalUi::new();

    assert!(
        handle_optional_user_config_setup_result(&ui, Err(io::Error::other("binding failed")))
            .is_ok()
    );
}

#[test]
fn optional_user_config_setup_preserves_interruption() {
    let ui = TerminalUi::new();

    let err = handle_optional_user_config_setup_result(
        &ui,
        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled")),
    )
    .unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::Interrupted);
}

fn test_config(paths: Vec<PathBuf>) -> config::Config {
    config::Config::with_roots(
        paths
            .into_iter()
            .map(|path| SearchRoot { path, depth: 1 })
            .collect(),
    )
}
