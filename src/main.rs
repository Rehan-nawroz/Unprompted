//! Unprompted entry point.
//!
//! - No arguments: opens the window.
//! - File or folder arguments (drag and drop onto the exe): opens the window with them queued.
//! - `--cli <files or folders>`: headless mode. Prints a report and sets the exit code
//!   (0 = all cleaned, 1 = some failed, 2 = nothing to do).

#![windows_subsystem = "windows"]

mod gui;
mod theme;
mod icons;

use std::path::PathBuf;

use unprompted::{clean_file, expand_inputs, summarize};

const HELP: &str = "\
Unprompted - strip AI prompts, EXIF, XMP and other metadata from images.

Usage:
  Unprompted.exe                       open the window
  Unprompted.exe <files or folders>    open the window with these queued
  Unprompted.exe --cli <files...>      headless: clean files and print a report

Output is written next to each source as <name>_clean.<ext>. Originals are not changed.
Supported: PNG, JPEG, WebP.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        Some("--cli") => std::process::exit(run_cli(&args[1..])),
        Some("-h") | Some("--help") => {
            print!("{HELP}");
        }
        _ => {
            
            let initial: Vec<PathBuf> = args.iter().map(PathBuf::from).collect();
            if let Err(e) = gui::run(initial) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn run_cli(args: &[String]) -> i32 {
    let inputs = expand_inputs(&args.iter().map(PathBuf::from).collect::<Vec<_>>());
    if inputs.is_empty() {
        eprintln!("no supported images given\n\n{HELP}");
        return 2;
    }
    let mut failures = 0usize;
    for path in &inputs {
        match clean_file(path) {
            Ok(o) => {
                let parts: Vec<String> = summarize(&o.cleaned.removed)
                    .iter()
                    .map(|s| format!("{} x{}", s.kind.label(), s.count))
                    .collect();
                let removed = if parts.is_empty() { "nothing to remove".to_string() } else { parts.join(", ") };
                println!(
                    "OK    {} -> {}  [{}]  {} -> {} bytes",
                    path.display(),
                    o.output.display(),
                    removed,
                    o.original_size,
                    o.cleaned_size()
                );
            }
            Err(e) => {
                failures += 1;
                println!("ERROR {}: {e}", path.display());
            }
        }
    }
    if failures > 0 {
        1
    } else {
        0
    }
}
