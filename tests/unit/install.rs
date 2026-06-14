use super::*;

#[test]
fn shell_quote_handles_single_quotes() {
    assert_eq!(shell_quote("/tmp/it's/tmuxxer"), "'/tmp/it'\\''s/tmuxxer'");
}
