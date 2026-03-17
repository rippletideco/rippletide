use std::path::Path;

pub fn count_rules_in_claude_md(cwd: &Path) -> usize {
    let path = cwd.join("CLAUDE.md");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("- ") || trimmed.starts_with("* ") || is_numbered_item(trimmed)
        })
        .count()
}

fn is_numbered_item(line: &str) -> bool {
    let mut chars = line.chars();
    // Must start with a digit
    match chars.next() {
        Some(c) if c.is_ascii_digit() => {}
        _ => return false,
    };
    // Consume remaining digits
    loop {
        match chars.next() {
            Some(c) if c.is_ascii_digit() => continue,
            Some('.') => break,
            _ => return false,
        }
    }
    // Must have a space after the dot
    matches!(chars.next(), Some(' '))
}
