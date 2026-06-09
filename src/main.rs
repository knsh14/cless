use std::env;
use std::fs;
use std::process::ExitCode;

mod highlight;
mod pager;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut wrap = true;
    let mut path: Option<&str> = None;
    for arg in &args[1..] {
        match arg.as_str() {
            "-h" | "--help" => {
                eprintln!("usage: cless [-S] <file>");
                return ExitCode::from(2);
            }
            // -S: chop long lines (disable wrapping), like less.
            "-S" => wrap = false,
            other if other.starts_with('-') && other.len() > 1 => {
                eprintln!("cless: unknown option: {}", other);
                return ExitCode::from(2);
            }
            other => {
                if path.is_some() {
                    eprintln!("usage: cless [-S] <file>");
                    return ExitCode::from(2);
                }
                path = Some(other);
            }
        }
    }
    let Some(path) = path else {
        eprintln!("usage: cless [-S] <file>");
        return ExitCode::from(2);
    };

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cless: {}: {}", path, e);
            return ExitCode::from(1);
        }
    };

    let lines = highlight::highlight_file(&content, path);

    if let Err(e) = pager::run(path, &lines, wrap) {
        eprintln!("cless: {}", e);
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
