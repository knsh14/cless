use std::path::Path;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};
use unicode_width::UnicodeWidthChar;

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

#[derive(Clone)]
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
    Cpp,
    Java,
    Ruby,
    Php,
    CSharp,
    Lua,
    Zig,
    Make,
    Hcl,
    Xml,
    Ini,
    Kotlin,
    Swift,
    Sql,
}

impl Lang {
    #[cfg(test)]
    const ALL: &'static [Lang] = &[
        Lang::Rust,
        Lang::Python,
        Lang::JavaScript,
        Lang::TypeScript,
        Lang::Tsx,
        Lang::Json,
        Lang::Go,
        Lang::Bash,
        Lang::Toml,
        Lang::Yaml,
        Lang::Html,
        Lang::Css,
        Lang::C,
        Lang::Markdown,
        Lang::Cpp,
        Lang::Java,
        Lang::Ruby,
        Lang::Php,
        Lang::CSharp,
        Lang::Lua,
        Lang::Zig,
        Lang::Make,
        Lang::Hcl,
        Lang::Xml,
        Lang::Ini,
        Lang::Kotlin,
        Lang::Swift,
        Lang::Sql,
    ];

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
            Lang::Cpp => "cpp",
            Lang::Java => "java",
            Lang::Ruby => "ruby",
            Lang::Php => "php",
            Lang::CSharp => "c_sharp",
            Lang::Lua => "lua",
            Lang::Zig => "zig",
            Lang::Make => "make",
            Lang::Hcl => "hcl",
            Lang::Xml => "xml",
            Lang::Ini => "ini",
            Lang::Kotlin => "kotlin",
            Lang::Swift => "swift",
            Lang::Sql => "sql",
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
        "cpp" | "cc" | "cxx" | "c++" | "hpp" | "hh" | "hxx" => return Some(Lang::Cpp),
        "java" => return Some(Lang::Java),
        "rb" | "rake" | "gemspec" => return Some(Lang::Ruby),
        "php" | "phtml" => return Some(Lang::Php),
        "cs" => return Some(Lang::CSharp),
        "lua" => return Some(Lang::Lua),
        "zig" => return Some(Lang::Zig),
        "mk" | "mak" => return Some(Lang::Make),
        "hcl" | "tf" | "tfvars" => return Some(Lang::Hcl),
        "xml" | "svg" | "xsl" | "xslt" | "xsd" | "plist" => return Some(Lang::Xml),
        "ini" | "cfg" => return Some(Lang::Ini),
        "kt" | "kts" => return Some(Lang::Kotlin),
        "swift" => return Some(Lang::Swift),
        "sql" => return Some(Lang::Sql),
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
    match file_name.as_str() {
        "makefile" | "gnumakefile" => return Some(Lang::Make),
        "gemfile" | "rakefile" => return Some(Lang::Ruby),
        _ => {}
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
        if rest.contains("ruby") {
            return Some(Lang::Ruby);
        }
        if rest.contains("lua") {
            return Some(Lang::Lua);
        }
        if rest.contains("php") {
            return Some(Lang::Php);
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

// tree-sitter-kotlin-ng and tree-sitter-hcl ship no highlights query, so a
// compact one is vendored here. Only node types and tokens verified present in
// each grammar are referenced; an unknown name would make the query fail to
// compile and silently drop the language back to plain text.
const KOTLIN_HIGHLIGHTS: &str = r#"
[
  "fun" "val" "var" "class" "object" "interface" "if" "else" "when"
  "for" "while" "do" "return" "throw" "try" "catch" "finally"
  "import" "package" "typealias" "in" "is" "as" "by" "where"
  "enum" "sealed" "data" "override" "private" "public" "protected"
  "internal" "abstract" "final" "open" "suspend" "companion"
  "constructor" "init" "get" "set"
] @keyword
["this" "super"] @variable.builtin
(user_type) @type
(line_comment) @comment
(block_comment) @comment
(shebang) @comment
(string_literal) @string
(multiline_string_literal) @string
(character_literal) @string
(escape_sequence) @string.escape
(number_literal) @number
(float_literal) @number
(label) @label
"#;

const HCL_HIGHLIGHTS: &str = r#"
(comment) @comment
(string_lit) @string
(template_literal) @string
(numeric_lit) @number
(bool_lit) @constant.builtin
(null_lit) @constant.builtin
(block (identifier) @type)
(function_call (identifier) @function)
(attribute (identifier) @property)
(get_attr (identifier) @property)
(variable_expr (identifier) @variable)
"#;

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
        Lang::Cpp => (
            tree_sitter_cpp::LANGUAGE.into(),
            tree_sitter_cpp::HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Lang::Java => (
            tree_sitter_java::LANGUAGE.into(),
            tree_sitter_java::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Ruby => (
            tree_sitter_ruby::LANGUAGE.into(),
            tree_sitter_ruby::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Php => (
            tree_sitter_php::LANGUAGE_PHP.into(),
            tree_sitter_php::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::CSharp => (
            tree_sitter_c_sharp::LANGUAGE.into(),
            tree_sitter_c_sharp::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Lua => (
            tree_sitter_lua::LANGUAGE.into(),
            tree_sitter_lua::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Zig => (
            tree_sitter_zig::LANGUAGE.into(),
            tree_sitter_zig::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Make => (
            tree_sitter_make::LANGUAGE.into(),
            tree_sitter_make::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Hcl => (tree_sitter_hcl::LANGUAGE.into(), HCL_HIGHLIGHTS, "", ""),
        Lang::Xml => (
            tree_sitter_xml::LANGUAGE_XML.into(),
            tree_sitter_xml::XML_HIGHLIGHT_QUERY,
            "",
            "",
        ),
        Lang::Ini => (
            tree_sitter_ini::LANGUAGE.into(),
            tree_sitter_ini::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Kotlin => (
            tree_sitter_kotlin_ng::LANGUAGE.into(),
            KOTLIN_HIGHLIGHTS,
            "",
            "",
        ),
        Lang::Swift => (
            tree_sitter_swift::LANGUAGE.into(),
            tree_sitter_swift::HIGHLIGHTS_QUERY,
            "",
            "",
        ),
        Lang::Sql => (
            tree_sitter_sequel::LANGUAGE.into(),
            tree_sitter_sequel::HIGHLIGHTS_QUERY,
            "",
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
                // Bytes emitted so far: a mid-stream error must not silently
                // truncate the file, so the un-emitted tail is appended plain.
                let mut consumed = 0usize;
                for event in events {
                    match event {
                        Ok(HighlightEvent::HighlightStart(h)) => stack.push(h.0),
                        Ok(HighlightEvent::HighlightEnd) => {
                            stack.pop();
                        }
                        Ok(HighlightEvent::Source { start, end }) => {
                            let text = content.get(start..end).unwrap_or("");
                            let style = stack
                                .last()
                                .and_then(|&i| HIGHLIGHT_NAMES.get(i).copied())
                                .map(|name| Style {
                                    foreground: color_for(name),
                                })
                                .unwrap_or(default_style);
                            push_text(&mut current, &mut lines, text, style);
                            consumed = end.max(consumed);
                        }
                        Err(_) => break,
                    }
                }
                if consumed < content.len() {
                    let tail = content.get(consumed..).unwrap_or("");
                    push_text(&mut current, &mut lines, tail, default_style);
                }
            }
            Err(_) => push_text(&mut current, &mut lines, content, default_style),
        }
    } else {
        push_text(&mut current, &mut lines, content, default_style);
    }

    lines.push(current);
    // Drop the phantom empty line a trailing newline produces (but keep the
    // single empty line of empty input).
    if lines.len() >= 2 && content.ends_with('\n') && lines.last().is_some_and(|l| l.is_empty()) {
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
        // Display column at the end of the line so far: tab stops depend on
        // it, and a span can start mid-line.
        let mut col: usize = current
            .iter()
            .flat_map(|(_, t)| t.chars())
            .map(|c| UnicodeWidthChar::width(c).unwrap_or(0))
            .sum();
        let mut cleaned = String::new();
        for ch in piece.chars() {
            match ch {
                '\r' => {}
                // Expand tabs to 8-column stops here so the renderer (which
                // draws only width>0 chars) and wrap math see real columns.
                '\t' => {
                    let pad = 8 - col % 8;
                    for _ in 0..pad {
                        cleaned.push(' ');
                    }
                    col += pad;
                }
                c => {
                    cleaned.push(c);
                    col += UnicodeWidthChar::width(c).unwrap_or(0);
                }
            }
        }
        if !cleaned.is_empty() {
            current.push((style, cleaned));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|(_, t)| t.as_str()).collect())
            .collect()
    }

    /// Tabs must be expanded to 8-column stops at load time; the renderer
    /// draws only width>0 chars, so a raw tab would vanish (and take the
    /// indentation of tab-indented files — Go, Makefiles — with it).
    #[test]
    fn tabs_expand_to_tab_stops() {
        let lines = highlight_file("a\tb\n\tx\n", "");
        assert_eq!(plain(&lines), vec!["a       b".to_string(), "        x".to_string()]);
    }

    /// Tab stops are computed from the display column, not the byte offset,
    /// so a wide char before the tab shifts the expansion.
    #[test]
    fn tab_stops_use_display_columns() {
        let lines = highlight_file("あ\tb", ""); // 'あ' is width 2 -> tab fills to col 8
        assert_eq!(plain(&lines), vec!["あ      b".to_string()]);
    }

    /// Tab expansion must also apply on the highlighted path (spans from
    /// tree-sitter events), not just the plain-text fallback.
    #[test]
    fn tabs_expand_in_highlighted_source() {
        let lines = highlight_file("fn main() {\n\tlet x = 1;\n}\n", "demo.rs");
        assert_eq!(plain(&lines)[1], "        let x = 1;");
    }

    /// Every language must build a valid HighlightConfiguration. This catches a
    /// crate whose query constant was renamed, a grammar/core ABI mismatch, or a
    /// vendored query (Kotlin, HCL) that references a node type the grammar does
    /// not have — all of which otherwise degrade silently to plain text.
    #[test]
    fn every_language_builds_a_config() {
        for &lang in Lang::ALL {
            assert!(
                build_config(lang).is_some(),
                "build_config failed for {}",
                lang.name()
            );
        }
    }

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

    fn distinct_colors(lines: &[Line]) -> usize {
        lines
            .iter()
            .flat_map(|l| &l.spans)
            .map(|(s, _)| (s.foreground.r, s.foreground.g, s.foreground.b))
            .collect::<std::collections::HashSet<_>>()
            .len()
    }

    /// The Kotlin and HCL highlight queries are hand-vendored (their crates ship
    /// none), so a query that compiles but matches nothing would still leave the
    /// file uncolored. Assert real code actually produces more than one color.
    #[test]
    fn vendored_queries_highlight_real_code() {
        let kotlin = "fun main() {\n    val x = 42\n    // hi\n}\n";
        assert!(
            distinct_colors(&highlight_file(kotlin, "demo.kt")) > 1,
            "kotlin produced no highlights"
        );

        let hcl = "resource \"aws_s3_bucket\" \"b\" {\n  bucket = \"x\" # note\n  count  = 3\n}\n";
        assert!(
            distinct_colors(&highlight_file(hcl, "demo.tf")) > 1,
            "hcl produced no highlights"
        );
    }
}
