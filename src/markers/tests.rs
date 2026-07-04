use super::*;

#[test]
fn block_span_includes_trailing_newline() {
    let content = "before\n# >>> tmuxxer >>>\nold\n# <<< tmuxxer <<<\nafter\n";

    let (start, end) = find_block_span(content).unwrap();

    assert_eq!(
        &content[start..end],
        "# >>> tmuxxer >>>\nold\n# <<< tmuxxer <<<\n"
    );
}

#[test]
fn upsert_replaces_existing_block_in_place() {
    let mut content = "before\n# >>> tmuxxer >>>\nold\n# <<< tmuxxer <<<\nafter\n".to_string();

    upsert_block(&mut content, "# >>> tmuxxer >>>\nnew\n# <<< tmuxxer <<<\n");

    assert_eq!(
        content,
        "before\n# >>> tmuxxer >>>\nnew\n# <<< tmuxxer <<<\nafter\n"
    );
}

#[test]
fn upsert_appends_block_after_final_newline() {
    let mut content = "existing\n".to_string();

    upsert_block(&mut content, "# >>> tmuxxer >>>\n# <<< tmuxxer <<<\n");

    assert_eq!(content, "existing\n# >>> tmuxxer >>>\n# <<< tmuxxer <<<\n");
}

#[test]
fn upsert_adds_missing_newline_before_appending() {
    let mut content = "no trailing newline".to_string();

    upsert_block(&mut content, "# >>> tmuxxer >>>\n# <<< tmuxxer <<<\n");

    assert_eq!(
        content,
        "no trailing newline\n# >>> tmuxxer >>>\n# <<< tmuxxer <<<\n"
    );
}

#[test]
fn upsert_into_empty_content_adds_no_leading_newline() {
    let mut content = String::new();

    upsert_block(&mut content, "# >>> tmuxxer >>>\n# <<< tmuxxer <<<\n");

    assert_eq!(content, "# >>> tmuxxer >>>\n# <<< tmuxxer <<<\n");
}
