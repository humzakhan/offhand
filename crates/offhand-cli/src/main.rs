//! `offhand run` drives the same local runner as the desktop shell.

use clap::{Parser, Subcommand};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use {
    offhand_core::{Agent, ConfirmationDecision, UiState},
    offhand_infer::NoopEngine,
    offhand_platform::Platform,
    std::{
        io::{self, IsTerminal, Write},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    },
};

/// Offhand: an offline-first computer-use agent.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Execute one accessibility command, for example: open Calculator.
    Run { command: String },
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let Command::Run { command } = cli.command;
    std::process::ExitCode::from(run(&command))
}

#[cfg(target_os = "macos")]
fn run(command: &str) -> u8 {
    drive(offhand_macos::MacPlatform, command)
}

#[cfg(target_os = "windows")]
fn run(command: &str) -> u8 {
    drive(offhand_windows::WindowsPlatform, command)
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn run(_command: &str) -> u8 {
    eprintln!("Offhand supports macOS and Windows; this operating system is unsupported.");
    2
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn drive(platform: impl Platform, command: &str) -> u8 {
    let stopped = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&stopped);
    if let Err(error) = ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst)) {
        eprintln!("Cannot install stop handler: {error}");
        return 1;
    }
    let mut agent = Agent::new(platform, NoopEngine);
    if let Err(error) = agent.submit(command) {
        eprintln!("{error}");
        return 1;
    }
    // Confirmation input is read on another thread so Ctrl+C can cancel while
    // waiting for input. Native platform objects remain on this thread.
    let (answers, input) = std::sync::mpsc::channel();
    let mut reading = false;
    let mut previous = None;
    loop {
        if stopped.load(Ordering::SeqCst) {
            agent.stop();
            return 130;
        }
        let state = agent.state().clone();
        if previous.as_ref() != Some(&state) {
            print_state(&state);
            previous = Some(state.clone());
        }
        match state {
            UiState::Done { .. } => return 0,
            UiState::Stuck { .. } => return 1,
            UiState::Blocked { .. } => return 2,
            UiState::Idle | UiState::Paused => return 130,
            UiState::AwaitConfirm {
                confirmation_id,
                label,
                target,
                ..
            } => {
                if !io::stdin().is_terminal() {
                    eprintln!("Confirmation denied: interactive input is required");
                    agent.stop();
                    return 130;
                }
                if !reading {
                    // No direct libc dependency is available for portable terminal
                    // input flushing. Approval still requires an interactive TTY.
                    let target_label = target
                        .as_ref()
                        .map_or(label.as_str(), |target| target.label.as_str());
                    print!(
                        "Type allow to approve {label} (target: {target_label}), or anything else to stop: "
                    );
                    if io::stdout().flush().is_err() {
                        agent.stop();
                        return 1;
                    }
                    let answers = answers.clone();
                    std::thread::spawn(move || {
                        let mut answer = String::new();
                        let allowed =
                            io::stdin().read_line(&mut answer).is_ok() && answer.trim() == "allow";
                        let _ = answers.send(allowed);
                    });
                    reading = true;
                }
                if let Ok(allowed) = input.try_recv() {
                    reading = false;
                    let decision = if allowed {
                        ConfirmationDecision::Allow
                    } else {
                        ConfirmationDecision::Stop
                    };
                    if let Err(error) = agent.respond(confirmation_id, decision) {
                        eprintln!("{error}");
                        return 1;
                    }
                    if !allowed {
                        return 130;
                    }
                }
            }
            UiState::Running { .. } | UiState::Listening => {
                // The handler is checked before each tick, including after confirmation.
                if stopped.load(Ordering::SeqCst) {
                    agent.stop();
                    return 130;
                }
                if let Err(error) = agent.tick() {
                    eprintln!("{error}");
                    return 1;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(110));
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn print_state(state: &UiState) {
    match state {
        UiState::Running { target, .. } => {
            if let Some(target) = target {
                println!("Target: {}", target.label);
            } else {
                println!("Resolving command");
            }
        }
        UiState::AwaitConfirm { label, .. } => println!("Confirmation required: {label}"),
        UiState::Done { summary, .. } => println!("{summary}"),
        UiState::Stuck { query, .. } => eprintln!("Stuck: {query}"),
        UiState::Blocked { label, .. } => eprintln!("Blocked: {label}"),
        UiState::Idle | UiState::Listening | UiState::Paused => {}
    }
}
