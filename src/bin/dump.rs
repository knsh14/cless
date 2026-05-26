// Debug helper: highlight a file and dump ANSI to stdout (no pager).
// Usage: dump <file> [pattern]
use std::env;
use std::fs;

#[path = "../highlight.rs"]
mod highlight;

fn main() {
    let path = env::args().nth(1).expect("usage: dump <file> [pattern]");
    let pattern = env::args().nth(2);
    let content = fs::read_to_string(&path).unwrap();
    let lines = highlight::highlight_file(&content, &path);

    if let Some(pat) = pattern {
        let re = regex::RegexBuilder::new(&pat)
            .case_insensitive(pat.chars().all(|c| !c.is_uppercase()))
            .build()
            .expect("invalid regex");
        let mut found_any = false;
        for (i, line) in lines.iter().enumerate() {
            let plain: String = line.spans.iter().map(|(_, t)| t.as_str()).collect();
            if re.is_match(&plain) {
                found_any = true;
                println!("line {}: {}", i + 1, plain);
            }
        }
        if !found_any {
            println!("(no match)");
        }
        return;
    }

    for line in &lines {
        for (style, text) in &line.spans {
            let fg = style.foreground;
            print!("\x1b[38;2;{};{};{}m{}", fg.r, fg.g, fg.b, text);
        }
        println!("\x1b[0m");
    }
}
