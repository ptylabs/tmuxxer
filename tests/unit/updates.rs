use super::*;

#[test]
fn parses_update_state() {
    let state = parse_state(
        "last_check_at = 42\n\
         latest_version = 1.2.3\n\
         latest_url = https://example.test/release\n\
         dismissed_version = 1.2.2\n",
    )
    .unwrap();

    assert_eq!(state.last_check_at, 42);
    assert_eq!(state.latest_version.as_deref(), Some("1.2.3"));
    assert_eq!(
        state.latest_url.as_deref(),
        Some("https://example.test/release")
    );
    assert_eq!(state.dismissed_version.as_deref(), Some("1.2.2"));
}

#[test]
fn extracts_json_string_fields() {
    let body =
        r#"{"tag_name":"v0.2.0","html_url":"https://github.com/ptylabs/tmuxxer/releases/1"}"#;

    assert_eq!(
        json_string_field(body, "tag_name").as_deref(),
        Some("v0.2.0")
    );
    assert_eq!(
        json_string_field(body, "html_url").as_deref(),
        Some("https://github.com/ptylabs/tmuxxer/releases/1")
    );
}

#[test]
fn compares_versions() {
    assert!(version_is_newer("0.2.0", "0.1.0-beta.3"));
    assert!(version_is_newer("0.1.0", "0.1.0-beta.3"));
    assert!(!version_is_newer("0.1.0-beta.3", "0.1.0-beta.3"));
    assert!(!version_is_newer("0.1.0-beta.2", "0.1.0-beta.3"));
}

#[test]
fn compares_pre_release_versions_with_semver_precedence() {
    assert!(version_is_newer("0.1.0-beta.10", "0.1.0-beta.4"));
    assert!(!version_is_newer("0.1.0-beta.4", "0.1.0-beta.10"));
    assert!(version_is_newer("1.0.0-alpha.1", "1.0.0-alpha"));
    assert!(version_is_newer("1.0.0-alpha.beta", "1.0.0-alpha.1"));
    assert!(version_is_newer("1.0.0-beta.11", "1.0.0-beta.2"));
    assert!(version_is_newer("1.0.0", "1.0.0-rc.1"));
    assert!(!version_is_newer("1.0.0+build.2", "1.0.0+build.1"));
}
