use super::*;

#[test]
fn tmux_binding_forwards_ctrl_f_to_current_pane() {
    let line = forward_ctrl_f_bind_line();

    assert_eq!(line, "bind-key -n C-f send-keys C-f");
    assert!(!line.contains("sessionize"));
    assert!(!line.contains("display-popup"));
    assert!(!line.contains("run-shell"));
}

#[test]
fn removes_legacy_literal_sessionize_binding() {
    let content = "\
set -g mouse on
# >>> tmuxxer >>>
bind-key -n C-f send-keys C-u \\; send-keys -l \"'tmuxxer' sessionize\" \\; send-keys Enter
# <<< tmuxxer <<<
";

    let cleaned = remove_legacy_ctrl_f_bindings(content);

    assert!(cleaned.contains(MARKER_START));
    assert!(cleaned.contains(MARKER_END));
    assert!(!cleaned.contains("send-keys -l"));
    assert!(!cleaned.contains("sessionize"));
}

#[test]
fn removes_old_tmux_helper_binding_with_comment() {
    let content = "\
set -g mouse on
# tmux-helper sessionizer
bind-key -n C-f run-shell -b \"/home/me/.cargo/bin/tmuxxer\"
set -g history-limit 50000
";

    let cleaned = remove_legacy_ctrl_f_bindings(content);

    assert_eq!(
        cleaned,
        "\
set -g mouse on
set -g history-limit 50000
"
    );
}

#[test]
fn keeps_unrelated_ctrl_f_binding() {
    let content = "\
bind-key -n C-f send-keys C-f
bind-key -n C-g run-shell -b \"tmuxxer sessionize\"
bind-key -n C-f run-shell -b \"tmux-sessionizer\"
";

    let cleaned = remove_legacy_ctrl_f_bindings(content);

    assert_eq!(cleaned, content);
}
