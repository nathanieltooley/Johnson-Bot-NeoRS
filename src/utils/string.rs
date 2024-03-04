pub fn string_char_isspace(str: &str, index: usize) -> bool {
    if let Some(c) = str.chars().nth(index) {
        if c == '\u{20}' || c == '\u{9}' {
            return true;
        }
    }

    false
}
