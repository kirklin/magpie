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

        if cleaned.len() <= max_len {
            cleaned
        } else {
            format!("{}…", &cleaned[..max_len])
        }
    }
}

impl Default for ContentClassifier {
    fn default() -> Self {
        Self::new()
    }
}
