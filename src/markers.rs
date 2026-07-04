pub const MARKER_START: &str = "# >>> tmuxxer >>>";
pub const MARKER_END: &str = "# <<< tmuxxer <<<";

/// Byte span of the tmuxxer marker block, including its trailing newline.
pub fn find_block_span(content: &str) -> Option<(usize, usize)> {
    let start = content.find(MARKER_START)?;
    let rest = &content[start..];
    let end_rel = rest.find(MARKER_END)? + MARKER_END.len();
    let end = start + end_rel;
    let end = if content[end..].starts_with('\n') {
        end + 1
    } else {
        end
    };
    Some((start, end))
}

/// Replace the existing marker block in place, or append the block at the end.
pub fn upsert_block(content: &mut String, block: &str) {
    if let Some((start, end)) = find_block_span(content) {
        content.replace_range(start..end, block);
        return;
    }
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(block);
}

#[cfg(test)]
mod tests;
