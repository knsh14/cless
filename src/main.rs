use std::env;
use std::fs;
use std::io::{IsTerminal, Read};
use std::process::ExitCode;

mod highlight;
mod pager;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut wrap = true;
    let mut numbers = false;
    let mut path: Option<&str> = None;
    for arg in &args[1..] {
        match arg.as_str() {
            "-h" | "--help" => {
                eprintln!("usage: cless [-S] [-N] <file>");
                return ExitCode::from(2);
            }
            // -S: chop long lines (disable wrapping), like less.
            "-S" => wrap = false,
            // -N: show line numbers, like less.
            "-N" => numbers = true,
            other if other.starts_with('-') && other.len() > 1 => {
                eprintln!("cless: unknown option: {}", other);
                return ExitCode::from(2);
            }
            other => {
                if path.is_some() {
                    eprintln!("usage: cless [-S] [-N] <file>");
                    return ExitCode::from(2);
                }
                path = Some(other);
            }
        }
    }
    // Read from stdin when given `-`, or no file with stdin piped in. The
    // display name is "(stdin)" and language detection falls back to the
    // shebang/content since there is no path. `detect_path` feeds extension
    // detection in highlight_file.
    let read_stdin = matches!(path, Some("-"))
        || (path.is_none() && !std::io::stdin().is_terminal());

    let (content, name, detect_path) = if read_stdin {
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
            eprintln!("cless: <stdin>: {}", e);
            return ExitCode::from(1);
        }
        (buf, "(stdin)".to_string(), "")
    } else {
        let Some(path) = path else {
            eprintln!("usage: cless [-S] [-N] <file>");
            return ExitCode::from(2);
        };
        match fs::read_to_string(path) {
            Ok(c) => (c, path.to_string(), path),
            Err(e) => {
                eprintln!("cless: {}: {}", path, e);
                return ExitCode::from(1);
            }
        }
    };

    let lines = highlight::highlight_file(&content, detect_path);

    if let Err(e) = pager::run(name, lines, wrap, numbers) {
        eprintln!("cless: {}", e);
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
