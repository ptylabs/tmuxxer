use super::*;

#[test]
fn resolver_accepts_durable_path_candidate() {
    let path = PathBuf::from("/home/user/.cargo/bin/tmuxxer");

    assert_eq!(
        resolve_tmuxxer_from(Some(path.clone()), None).unwrap(),
        path
    );
}

#[test]
fn resolver_rejects_target_debug_path() {
    let err =
        resolve_tmuxxer_from(Some(PathBuf::from("/repo/target/debug/tmuxxer")), None).unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::NotFound);
}

#[test]
fn resolver_rejects_target_release_path() {
    let err = resolve_tmuxxer_from(Some(PathBuf::from("/repo/target/release/tmuxxer")), None)
        .unwrap_err();

    assert_eq!(err.kind(), io::ErrorKind::NotFound);
}

#[test]
fn resolver_accepts_durable_current_exe() {
    let path = PathBuf::from("/opt/tmuxxer/tmuxxer");

    assert_eq!(
        resolve_tmuxxer_from(None, Some(path.clone())).unwrap(),
        path
    );
}

#[test]
fn resolver_uses_current_exe_after_unstable_path_candidate() {
    let path = PathBuf::from("/opt/tmuxxer/tmuxxer");

    assert_eq!(
        resolve_tmuxxer_from(
            Some(PathBuf::from("/repo/target/debug/tmuxxer")),
            Some(path.clone())
        )
        .unwrap(),
        path
    );
}

#[test]
fn shell_quote_handles_single_quotes() {
    assert_eq!(shell_quote("/tmp/it's/tmuxxer"), "'/tmp/it'\\''s/tmuxxer'");
}
