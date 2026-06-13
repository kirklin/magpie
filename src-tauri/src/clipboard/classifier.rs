use regex::Regex;

/// Content type classifier: auto-detects the type of clipboard content
pub struct ContentClassifier {
    url_regex: Regex,
    email_regex: Regex,
    hex_color_regex: Regex,
    rgb_color_regex: Regex,
    hsl_color_regex: Regex,
}

impl ContentClassifier {
    pub fn new() -> Self {
        Self {
            url_regex: Regex::new(r"^https?://[^\s]+$").unwrap(),
            email_regex: Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap(),
            hex_color_regex: Regex::new(r"^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$")
                .unwrap(),
            rgb_color_regex: Regex::new(
                r"^rgba?\(\s*\d{1,3}\s*,\s*\d{1,3}\s*,\s*\d{1,3}\s*(,\s*[\d.]+\s*)?\)$",
            )
            .unwrap(),
            hsl_color_regex: Regex::new(
                r"^hsla?\(\s*\d{1,3}\s*,\s*\d{1,3}%?\s*,\s*\d{1,3}%?\s*(,\s*[\d.]+\s*)?\)$",
            )
            .unwrap(),
        }
    }

    /// Classify text content into a specific type
    pub fn classify_text(&self, text: &str) -> &str {
        let trimmed = text.trim();

        // Check for URL
        if self.url_regex.is_match(trimmed) {
            return "url";
        }

        // Check for email
        if self.email_regex.is_match(trimmed) {
            return "email";
        }

        // Check for color values
        if self.hex_color_regex.is_match(trimmed)
            || self.rgb_color_regex.is_match(trimmed)
            || self.hsl_color_regex.is_match(trimmed)
        {
            return "color";
        }

        // Check for code-like content (heuristic)
        if self.looks_like_code(trimmed) {
            return "code";
        }

        "text"
    }

    /// Heuristic check for code content
    fn looks_like_code(&self, text: &str) -> bool {
        let code_indicators = [
            "function ",
            "const ",
            "let ",
            "var ",
            "import ",
            "export ",
            "class ",
            "def ",
            "fn ",
            "pub ",
            "struct ",
            "impl ",
            "package ",
            "func ",
            "if (",
            "for (",
            "while (",
            "return ",
            "=> {",
            "-> {",
            "();",
            ");",
            "};",
        ];

        let line_count = text.lines().count();
        if line_count < 2 {
            return false;
        }

        let indicator_count = code_indicators
            .iter()
            .filter(|ind| text.contains(**ind))
            .count();

        // If multiple code indicators are found, it's likely code
        indicator_count >= 2
    }

    /// Generate a short preview for display in the list
    pub fn generate_preview(text: &str, max_len: usize) -> String {
        let cleaned = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<&str>>()
            .join(" ");

        // `max_len` counts characters, not bytes: slicing on a raw byte
        // index panics whenever it lands inside a multi-byte codepoint
        // (common for CJK content, where each char is 3 bytes).
        match cleaned.char_indices().nth(max_len) {
            Some((byte_idx, _)) => format!("{}…", &cleaned[..byte_idx]),
            None => cleaned,
        }
    }
}

impl Default for ContentClassifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_keeps_short_text_unchanged() {
        assert_eq!(ContentClassifier::generate_preview("hello world", 100), "hello world");
    }

    #[test]
    fn preview_collapses_lines_and_trims() {
        let input = "  first  \n\n  second  \n";
        assert_eq!(ContentClassifier::generate_preview(input, 100), "first second");
    }

    #[test]
    fn preview_truncates_ascii_on_char_boundary() {
        let input = "a".repeat(150);
        let preview = ContentClassifier::generate_preview(&input, 100);
        assert_eq!(preview, format!("{}…", "a".repeat(100)));
    }

    #[test]
    fn preview_does_not_panic_on_multibyte_boundary() {
        // Each '汉' is 3 bytes; byte index 100 lands inside a codepoint.
        // The old byte-slice implementation panicked here.
        let input = "汉".repeat(150);
        let preview = ContentClassifier::generate_preview(&input, 100);
        assert_eq!(preview, format!("{}…", "汉".repeat(100)));
        assert_eq!(preview.chars().count(), 101); // 100 chars + ellipsis
    }
}
