use super::*;

#[test]
fn bashrc_block_uses_runtime_home_not_install_home() {
    let block = bashrc_block();

    assert!(block.contains("${XDG_CONFIG_HOME:-$HOME/.config}/tmuxxer/bash-bind.sh"));
    assert!(!block.contains("/home/"));
}

#[test]
fn bind_script_runs_inside_and_outside_tmux_without_echoing_command() {
    let script = bind_script_body("'tmuxxer' sessionize");

    assert!(script.contains("_tmuxxer_sessionize()"));
    assert!(script.contains("'tmuxxer' sessionize"));
    assert!(script.contains("bind -x '\"\\C-f\": \"_tmuxxer_sessionize\"'"));
    assert!(!script.contains("TMUX"));
}
