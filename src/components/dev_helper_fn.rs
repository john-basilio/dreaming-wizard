//! This module is simply a pool for shared helper functions.

/// Truncates `s` to at most `char_limit` characters, appending `...` when it
/// had to cut anything. Used anywhere a `StoryNode` title or `Character`
/// name is rendered in a tight space (the on-canvas node label, and both
/// editors' headers).
pub fn display_title(s: &str, char_limit: usize) -> String {
    let count = s.chars().count();

    if count <= char_limit {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(char_limit -1).collect();
        format!("{truncated}...")
    }
}
