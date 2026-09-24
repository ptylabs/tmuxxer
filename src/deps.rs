use std::env;
use std::path::Path;

pub fn ensure_tools() -> Result<(), String> {
    let search_path = env::var_os("PATH").unwrap_or_default();
    let mut tmux = false;
    let mut fzf = false;
    for dir in env::split_paths(&search_path) {
        tmux = tmux || is_executable(&dir.join("tmux"));
        fzf = fzf || is_executable(&dir.join("fzf"));
        if tmux && fzf {
            return Ok(());
        }
    }
    let missing = if tmux { "fzf" } else { "tmux" };
    Err(format!("{missing} not found on PATH (install {missing})"))
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn dependency_must_be_an_executable_file() {
        let temp = crate::test_support::TempDir::new("tmuxxer-deps");
        let file = temp.join("tool");
        assert!(!is_executable(&file));
        assert!(!is_executable(temp.path()));
        std::fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!is_executable(&file));
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(is_executable(&file));
        let link = temp.join("link");
        symlink(&file, &link).unwrap();
        assert!(is_executable(&link));
    }
}
