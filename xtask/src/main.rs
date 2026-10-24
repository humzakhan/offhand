//! Developer tasks, run with `cargo xtask <task>`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    if let Some(t @ ("fetch-model" | "bench" | "parity-check")) = task.as_deref() {
        println!("xtask {t}: TODO");
        ExitCode::SUCCESS
    } else {
        eprintln!("usage: cargo xtask <fetch-model|bench|parity-check>");
        ExitCode::FAILURE
    }
}
