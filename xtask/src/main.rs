//! Developer tasks, run with `cargo xtask <task>`.

mod fetch_model;

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    if task.as_deref() == Some("fetch-model") {
        match fetch_model::run(std::env::args().nth(2).as_deref()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("fetch-model: {error}");
                ExitCode::FAILURE
            }
        }
    } else if let Some(t @ ("bench" | "parity-check")) = task.as_deref() {
        println!("xtask {t}: TODO");
        ExitCode::SUCCESS
    } else {
        eprintln!("usage: cargo xtask <fetch-model|bench|parity-check>");
        ExitCode::FAILURE
    }
}
