use super::*;

#[test]
fn shell_from_path_detects_supported_shells() {
    assert_eq!(Shell::from_path("/bin/bash"), Some(Shell::Bash));
    assert_eq!(Shell::from_path("-zsh"), Some(Shell::Zsh));
    assert_eq!(Shell::from_path("/usr/bin/fish"), Some(Shell::Fish));
    assert_eq!(Shell::from_path("/usr/bin/fsh"), Some(Shell::Fish));
    assert_eq!(Shell::from_path("/usr/bin/nu"), Some(Shell::Nushell));
    assert_eq!(Shell::from_path("/usr/bin/nushell"), Some(Shell::Nushell));
    assert_eq!(Shell::from_path("/sbin/nologin"), None);
}

#[test]
fn bash_block_uses_bind_x() {
    let block = binding_block(Shell::Bash, "'tmuxxer' sessionize");

    assert!(block.contains("if [[ $- == *i* ]]"));
    assert!(block.contains("_tmuxxer_sessionize()"));
    assert!(block.contains("'tmuxxer' sessionize"));
    assert!(block.contains("bind -x '\"\\C-f\": \"_tmuxxer_sessionize\"'"));
}

#[test]
fn zsh_block_uses_zle_widget() {
    let block = binding_block(Shell::Zsh, "'tmuxxer' sessionize");

    assert!(block.contains("zle -N _tmuxxer_sessionize"));
    assert!(block.contains("bindkey '^F' _tmuxxer_sessionize"));
    assert!(block.contains("bindkey -M viins '^F' _tmuxxer_sessionize"));
    assert!(block.contains("zle reset-prompt"));
}

#[test]
fn fish_block_uses_fish_bind() {
    let block = binding_block(Shell::Fish, "command 'tmuxxer' sessionize");

    assert!(block.contains("if status is-interactive"));
    assert!(block.contains("function _tmuxxer_sessionize"));
    assert!(block.contains("bind \\cf _tmuxxer_sessionize"));
    assert!(block.contains("bind -M insert \\cf _tmuxxer_sessionize"));
}

#[test]
fn nushell_block_appends_keybinding() {
    let block = binding_block(Shell::Nushell, "run-external \"tmuxxer\" sessionize");

    assert!(block.contains("upsert keybindings"));
    assert!(block.contains("get --optional keybindings"));
    assert!(block.contains("where name != \"tmuxxer_sessionize\""));
    assert!(block.contains("keycode: char_f"));
    assert!(block.contains("send: executehostcommand"));
}

#[test]
fn command_quoting_is_shell_specific() {
    let path = Path::new("/tmp/it's/tmuxxer");

    assert_eq!(
        shell_command(Shell::Bash, path),
        "'/tmp/it'\\''s/tmuxxer' sessionize"
    );
    assert_eq!(
        shell_command(Shell::Fish, path),
        "command '/tmp/it\\'s/tmuxxer' sessionize"
    );
    assert_eq!(
        shell_command(Shell::Nushell, path),
        "run-external \"/tmp/it's/tmuxxer\" sessionize"
    );
}

#[test]
fn block_span_includes_trailing_newline() {
    let content = "before\n# >>> tmuxxer >>>\nold\n# <<< tmuxxer <<<\nafter\n";

    let (start, end) = find_block_span(content).unwrap();

    assert_eq!(
        &content[start..end],
        "# >>> tmuxxer >>>\nold\n# <<< tmuxxer <<<\n"
    );
}
