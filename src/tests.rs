use super::*;

#[test]
fn help_text_keeps_continuation_indentation() {
    let help = help_text();

    assert!(help.contains("  tmuxxer config get KEY\n                       Print a config value"));
    assert!(help.contains("  tmuxxer --add PATH...\n                       Toggle search roots"));
}
