//! Repository automation for `openbimrs/ifcx`, run as `cargo run -p xtask -- <command>`.
//!
//! Everything the documentation site states that can be derived from the
//! source is generated here, and every generator has a `--check` mode that
//! the gate runs, so a page cannot silently drift from the code, manifest,
//! README or changelog it describes. Ported from `openbimrs/ifc`'s xtask,
//! cut down to what this workspace has.
//!
//! ```text
//! cargo run -p xtask -- docs           regenerate every generated docs file and region
//! cargo run -p xtask -- docs --check   fail if any of them is out of date
//! cargo run -p xtask -- todo           list every unfinished-work marker with its issue
//! cargo run -p xtask -- todo --check   fail on a marker that names no issue
//! ```

mod docs;
mod text;
mod todo;
mod workspace;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, rest) = match args.split_first() {
        Some((command, rest)) => (command.as_str(), rest),
        None => return usage(),
    };
    let check = match rest {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return usage(),
    };
    let result = match command {
        "docs" => docs::run(check),
        "todo" => todo::run(check),
        _ => return usage(),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask {command}: {error}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: cargo run -p xtask -- <docs|todo> [--check]");
    ExitCode::from(2)
}
