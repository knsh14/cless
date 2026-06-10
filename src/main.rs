use std::env;
use std::io::{IsTerminal, Read};
use std::process::ExitCode;

mod highlight;
mod pager;

use pager::Source;

const USAGE: &str = "usage: cless [-S] [-N] <file>...";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut wrap = true;
    let mut numbers = false;
    let mut paths: Vec<&str> = Vec::new();
    for arg in &args[1..] {
        match arg.as_str() {
            "-h" | "--help" => {
                eprintln!("{}", USAGE);
                return ExitCode::from(2);
            }
            // -S: chop long lines (disable wrapping), like less.
            "-S" => wrap = false,
            // -N: show line numbers, like less.
            "-N" => numbers = true,
            // A lone `-` means stdin; longer dash args are unknown options.
            other if other.starts_with('-') && other.len() > 1 => {
                eprintln!("cless: unknown option: {}", other);
                return ExitCode::from(2);
            }
            other => paths.push(other),
        }
    }

    // stdin: an explicit lone `-`, or no file with stdin piped in. crossterm
    // still reads keys from /dev/tty. stdin cannot be mixed with files (it
    // can't be re-read for `:n`/`:p`).
    let read_stdin =
        (paths.is_empty() && !std::io::stdin().is_terminal()) || (paths == ["-"]);
    if !read_stdin && paths.iter().any(|p| *p == "-") {
        eprintln!("cless: '-' (stdin) cannot be combined with files");
        return ExitCode::from(2);
    }

    let sources = if read_stdin {
        let mut buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
            eprintln!("cless: <stdin>: {}", e);
            return ExitCode::from(1);
        }
        // No path => language detection falls back to shebang/content.
        vec![Source::memory("(stdin)", highlight::highlight_file(&buf, ""))]
    } else {
        if paths.is_empty() {
            eprintln!("{}", USAGE);
            return ExitCode::from(2);
        }
        paths.iter().map(|p| Source::file(*p, *p)).collect()
    };

    if let Err(e) = pager::run(sources, wrap, numbers) {
        eprintln!("cless: {}", e);
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
