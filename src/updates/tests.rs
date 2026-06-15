use super::*;
use crate::test_support::TempDir;
use std::fs;

#[test]
fn parses_update_state() {
    let state = parse_state(
        "last_check_at = 42\n\
         latest_version = 1.2.3\n\
         latest_url = https://example.test/release\n\
         dismissed_version = 1.2.2\n\
         auto_check_disabled = true\n",
    )
    .unwrap();

    assert_eq!(state.last_check_at, 42);
    assert_eq!(state.latest_version.as_deref(), Some("1.2.3"));
    assert_eq!(
        state.latest_url.as_deref(),
        Some("https://example.test/release")
    );
    assert_eq!(state.dismissed_version.as_deref(), Some("1.2.2"));
    assert!(state.auto_check_disabled);
}

#[test]
fn parses_release_json() {
    let body =
        r#"{"tag_name":"v0.2.0","html_url":"https://github.com/ptylabs/tmuxxer/releases/1"}"#;

    let release = parse_release_details(body).unwrap();

    assert_eq!(release.version, "0.2.0");
    assert_eq!(release.url, "https://github.com/ptylabs/tmuxxer/releases/1");
}

#[test]
fn parses_release_assets() {
    let body = r#"{
        "tag_name": "v0.2.0",
        "html_url": "https://github.com/ptylabs/tmuxxer/releases/1",
        "assets": [
            {
                "name": "tmuxxer-0.2.0-x86_64-unknown-linux-gnu.tar.gz",
                "browser_download_url": "https://example.test/tmuxxer.tar.gz"
            }
        ]
    }"#;

    let release = parse_release_details(body).unwrap();

    assert_eq!(release.version, "0.2.0");
    assert_eq!(release.assets.len(), 1);
    assert_eq!(
        release.assets[0].name,
        "tmuxxer-0.2.0-x86_64-unknown-linux-gnu.tar.gz"
    );
}

#[test]
fn finds_release_asset_for_target() {
    let release = LatestRelease {
        version: "0.2.0".to_string(),
        url: RELEASES_URL.to_string(),
        assets: vec![ReleaseAsset {
            name: "tmuxxer-0.2.0-x86_64-unknown-linux-gnu.tar.gz".to_string(),
            url: "https://example.test/tmuxxer.tar.gz".to_string(),
        }],
    };

    let asset = find_release_asset(&release, "x86_64-unknown-linux-gnu").unwrap();

    assert_eq!(asset.url, "https://example.test/tmuxxer.tar.gz");
}

#[test]
fn finds_checksum_asset_for_release() {
    let release = LatestRelease {
        version: "0.2.0".to_string(),
        url: RELEASES_URL.to_string(),
        assets: vec![ReleaseAsset {
            name: "tmuxxer-0.2.0-sha256sums.txt".to_string(),
            url: "https://example.test/sha256sums.txt".to_string(),
        }],
    };

    let asset = find_checksum_asset(&release).unwrap();

    assert_eq!(asset.url, "https://example.test/sha256sums.txt");
}

#[test]
fn parses_checksum_for_asset() {
    let checksums = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  *tmuxxer-0.2.0-x86_64-unknown-linux-gnu.tar.gz\n\
                     bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  other.tar.gz\n";

    let checksum =
        checksum_for_asset(checksums, "tmuxxer-0.2.0-x86_64-unknown-linux-gnu.tar.gz").unwrap();

    assert_eq!(
        checksum,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
}

#[test]
fn rejects_unsafe_archive_entries() {
    assert!(archive_entry_is_unsafe("../tmuxxer"));
    assert!(archive_entry_is_unsafe("pkg/../tmuxxer"));
    assert!(archive_entry_is_unsafe("/tmp/tmuxxer"));
    assert!(!archive_entry_is_unsafe("tmuxxer"));
    assert!(!archive_entry_is_unsafe("tmuxxer-0.2.0/tmuxxer"));
}

#[test]
fn detects_cargo_install_path_from_cargo_root_metadata() {
    let dir = TempDir::new("tmuxxer-cargo-root");
    let root = dir.join("cargo");
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(root.join(".crates.toml"), "").unwrap();

    assert!(is_cargo_install_path(&bin.join("tmuxxer")));
}

#[test]
fn treats_non_cargo_path_as_script_install() {
    assert!(!is_cargo_install_path(&PathBuf::from(
        "/usr/local/bin/tmuxxer"
    )));
}

#[test]
fn fetch_latest_release_uses_fetcher_trait() {
    let fetcher = FakeFetcher {
        body: r#"{"name":"0.2.0"}"#,
    };

    let (version, url) = fetch_latest_release_with(&fetcher).unwrap();

    assert_eq!(version, "0.2.0");
    assert_eq!(url, RELEASES_URL);
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

struct FakeFetcher {
    body: &'static str,
}

impl ReleaseFetcher for FakeFetcher {
    fn fetch(&self, _url: &str) -> Result<String, UpdateError> {
        Ok(self.body.to_string())
    }
}
