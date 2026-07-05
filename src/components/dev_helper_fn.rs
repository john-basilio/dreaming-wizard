// This module is simply a pool for shared helper functions

pub fn display_title(s: &str, char_limit: usize) -> String {
    let count = s.chars().count();

    if count <= char_limit {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(char_limit -1).collect();
        format!("{}...", truncated)
    }
}
