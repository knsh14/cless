use std::env;
use std::fs;
use std::process::ExitCode;

mod highlight;
mod pager;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 || args[1] == "-h" || args[1] == "--help" {
        eprintln!("usage: cless <file>");
        return ExitCode::from(2);
    }
    let path = &args[1];

    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cless: {}: {}", path, e);
            return ExitCode::from(1);
        }
    };

    let lines = highlight::highlight_file(&content, path);

    if let Err(e) = pager::run(path, &lines) {
        eprintln!("cless: {}", e);
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
