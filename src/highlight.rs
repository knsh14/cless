use std::path::Path;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

#[derive(Clone, Copy, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Clone, Copy, Default)]
pub struct Style {
    pub foreground: Color,
}

pub struct Line {
    pub spans: Vec<(Style, String)>,
}

// citruszest palette (https://github.com/zootedb0t/citruszest.nvim).
const FG_DEFAULT: Color = c(0xBF, 0xBF, 0xBF);
const FG_COMMENT: Color = c(0x65, 0x9B, 0x75);
const FG_KEYWORD: Color = c(0x28, 0xC9, 0xFF);
const FG_FUNCTION: Color = c(0x1A, 0xFF, 0xA3);
const FG_TYPE: Color = c(0xFF, 0xAA, 0x54);
const FG_STRING: Color = c(0xB2, 0xF3, 0xAC);
const FG_NUMBER: Color = c(0xFF, 0xF2, 0xB3);
const FG_CONSTANT: Color = c(0xFF, 0x74, 0x31);
const FG_PROPERTY: Color = c(0x00, 0xCC, 0x7A);
const FG_VARIABLE: Color = c(0xFF, 0xFF, 0x00);
const FG_OPERATOR: Color = c(0xFF, 0xFF, 0x00);
const FG_PUNCT: Color = c(0xFF, 0xAA, 0x54);
const FG_ATTRIBUTE: Color = c(0xFF, 0x74, 0x31);
const FG_TAG: Color = c(0xFF, 0xAA, 0x54);
const FG_TAG_DELIM: Color = c(0xFF, 0x54, 0x54);
const FG_ESCAPE: Color = c(0xAF, 0x74, 0xEE);
const FG_LABEL: Color = c(0x1A, 0xFF, 0xA3);
const FG_PARAM: Color = c(0x9A, 0xDC, 0xFF);

const fn c(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b }
}

const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "constant.builtin",
    "escape",
    "function",
    "function.builtin",
    "function.macro",
    "keyword",
    "label",
    "number",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "string.escape",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
];

fn color_for(name: &str) -> Color {
    match name {
        "attribute" => FG_ATTRIBUTE,
        "comment" => FG_COMMENT,
        "number" => FG_NUMBER,
        "constant" | "constant.builtin" => FG_CONSTANT,
        "escape" | "string.escape" => FG_ESCAPE,
        "function" | "function.builtin" | "function.macro" => FG_FUNCTION,
        "keyword" => FG_KEYWORD,
        "label" => FG_LABEL,
        "operator" => FG_OPERATOR,
        "property" => FG_PROPERTY,
        "punctuation" | "punctuation.bracket" | "punctuation.special" => FG_PUNCT,
        "punctuation.delimiter" => FG_TAG_DELIM,
        "string" | "string.special" => FG_STRING,
        "tag" => FG_TAG,
        "type" | "type.builtin" => FG_TYPE,
        "variable" | "variable.builtin" => FG_VARIABLE,
        "variable.parameter" => FG_PARAM,
        _ => FG_DEFAULT,
    }
}

#[derive(Clone, Copy)]
enum Lang {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
    Json,
    Go,
    Bash,
    Toml,
    Yaml,
    Html,
    Css,
    C,
    Markdown,
}

impl Lang {
    fn name(self) -> &'static str {
        match self {
            Lang::Rust => "rust",
            Lang::Python => "python",
            Lang::JavaScript => "javascript",
            Lang::TypeScript => "typescript",
            Lang::Tsx => "tsx",
            Lang::Json => "json",
            Lang::Go => "go",
            Lang::Bash => "bash",
            Lang::Toml => "toml",
            Lang::Yaml => "yaml",
            Lang::Html => "html",
            Lang::Css => "css",
            Lang::C => "c",
            Lang::Markdown => "markdown",
        }
    }
}

fn detect_language(path: &str, content: &str) -> Option<Lang> {
    let p = Path::new(path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "rs" => return Some(Lang::Rust),
        "py" | "pyi" | "pyw" => return Some(Lang::Python),
        "js" | "mjs" | "cjs" | "jsx" => return Some(Lang::JavaScript),
        "ts" => return Some(Lang::TypeScript),
        "tsx" => return Some(Lang::Tsx),
        "json" | "jsonc" => return Some(Lang::Json),
        "go" => return Some(Lang::Go),
        "sh" | "bash" | "zsh" | "ksh" => return Some(Lang::Bash),
        "toml" => return Some(Lang::Toml),
        "yaml" | "yml" => return Some(Lang::Yaml),
        "html" | "htm" => return Some(Lang::Html),
        "css" => return Some(Lang::Css),
        "c" | "h" => return Some(Lang::C),
        "md" | "markdown" => return Some(Lang::Markdown),
        _ => {}
    }

    let file_name = p
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        file_name.as_str(),
        ".bashrc" | ".bash_profile" | ".bash_aliases" | ".zshrc" | ".profile"
    ) {
        return Some(Lang::Bash);
    }

    let first_line = content.lines().next().unwrap_or("");
    if let Some(rest) = first_line.strip_prefix("#!") {
        let rest = rest.trim();
        if rest.contains("python") {
            return Some(Lang::Python);
        }
        if rest.contains("node") || rest.contains("deno") {
            return Some(Lang::JavaScript);
        }
        if rest.contains("bash")
            || rest.contains("zsh")
            || rest.ends_with("/sh")
            || rest.contains("/sh ")
        {
            return Some(Lang::Bash);
        }
    }

    None
}

fn build_config(lang: Lang) -> Option<HighlightConfiguration> {
    let (language, hl, inj, locals): (tree_sitter::Language, &str, &str, &str) = match lang {
        Lang::Rust => (
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            tree_sitter_rust::INJECTIONS_QUERY,
            "",
        ),
        Lang::Python => (
            tree_sitter_python::LANGUAGE.into(),
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::JavaScript => (
            tree_sitter_javascript::LANGUAGE.into(),
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::INJECTIONS_QUERY,
            tree_sitter_javascript::LOCALS_QUERY,
        ),
        Lang::TypeScript => (
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            "",
            tree_sitter_typescript::LOCALS_QUERY,
        ),
        Lang::Tsx => (
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            "",
            tree_sitter_typescript::LOCALS_QUERY,
        ),
        Lang::Json => (
            tree_sitter_json::LANGUAGE.into(),
            tree_sitter_json::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Go => (
            tree_sitter_go::LANGUAGE.into(),
            tree_sitter_go::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Bash => (
            tree_sitter_bash::LANGUAGE.into(),
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Lang::Toml => (
            tree_sitter_toml_ng::LANGUAGE.into(),
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Yaml => (
            tree_sitter_yaml::LANGUAGE.into(),
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Html => (
            tree_sitter_html::LANGUAGE.into(),
            tree_sitter_html::HIGHLIGHTS_QUERY,
            tree_sitter_html::INJECTIONS_QUERY,
            "",
        ),
        Lang::Css => (
            tree_sitter_css::LANGUAGE.into(),
            tree_sitter_css::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::C => (
            tree_sitter_c::LANGUAGE.into(),
            tree_sitter_c::HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Lang::Markdown => (
            tree_sitter_md::LANGUAGE.into(),
            tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
            tree_sitter_md::INJECTION_QUERY_BLOCK,
            "",
        ),
    };
    let mut config = HighlightConfiguration::new(language, lang.name(), hl, inj, locals).ok()?;
    config.configure(HIGHLIGHT_NAMES);
    Some(config)
}

pub fn highlight_file(content: &str, path: &str) -> Vec<Line> {
    let lang = detect_language(path, content);
    let config = lang.and_then(build_config);

    let mut lines: Vec<Vec<(Style, String)>> = Vec::new();
    let mut current: Vec<(Style, String)> = Vec::new();
    let default_style = Style {
        foreground: FG_DEFAULT,
    };

    if let Some(config) = config {
        let mut highlighter = Highlighter::new();
        let mut stack: Vec<usize> = Vec::new();
        match highlighter.highlight(&config, content.as_bytes(), None, |_| None) {
            Ok(events) => {
                for event in events {
                    match event {
                        Ok(HighlightEvent::HighlightStart(h)) => stack.push(h.0),
                        Ok(HighlightEvent::HighlightEnd) => {
                            stack.pop();
                        }
                        Ok(HighlightEvent::Source { start, end }) => {
                            let text = &content[start..end];
                            let style = stack
                                .last()
                                .and_then(|&i| HIGHLIGHT_NAMES.get(i).copied())
                                .map(|name| Style {
                                    foreground: color_for(name),
                                })
                                .unwrap_or(default_style);
                            push_text(&mut current, &mut lines, text, style);
                        }
                        Err(_) => break,
                    }
                }
            }
            Err(_) => push_text(&mut current, &mut lines, content, default_style),
        }
    } else {
        push_text(&mut current, &mut lines, content, default_style);
    }

    lines.push(current);
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    if lines.len() >= 2
        && lines.last().map(|l| l.is_empty()).unwrap_or(false)
        && content.ends_with('\n')
    {
        lines.pop();
    }

    lines.into_iter().map(|spans| Line { spans }).collect()
}

fn push_text(
    current: &mut Vec<(Style, String)>,
    lines: &mut Vec<Vec<(Style, String)>>,
    text: &str,
    style: Style,
) {
    let mut first = true;
    for piece in text.split('\n') {
        if !first {
            lines.push(std::mem::take(current));
        }
        first = false;
        let cleaned: String = piece.chars().filter(|c| *c != '\r').collect();
        if !cleaned.is_empty() {
            current.push((style, cleaned));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test for the tree-sitter parse + highlight path: catches a broken
    /// grammar/core ABI after a tree-sitter dependency bump (the build alone
    /// only proves the Rust API matches, not that parsing produces highlights).
    #[test]
    fn highlights_rust_source() {
        let src = "fn main() {\n    let x = 42;\n}\n";
        let lines = highlight_file(src, "demo.rs");

        // Reconstructing the spans must reproduce the input exactly.
        let mut reconstructed = String::new();
        for line in &lines {
            for (_, text) in &line.spans {
                reconstructed.push_str(text);
            }
            reconstructed.push('\n');
        }
        assert_eq!(reconstructed, src);

        // More than one distinct foreground color => tree-sitter actually
        // highlighted tokens (keyword/number/etc.), not just the default.
        let colors: std::collections::HashSet<(u8, u8, u8)> = lines
            .iter()
            .flat_map(|l| &l.spans)
            .map(|(s, _)| (s.foreground.r, s.foreground.g, s.foreground.b))
            .collect();
        assert!(
            colors.len() > 1,
            "expected multiple highlight colors, got {:?}",
            colors
        );
    }
}
