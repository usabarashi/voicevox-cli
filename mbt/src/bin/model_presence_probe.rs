//! Probe used by `tests/model_presence.rs`.
//!
//! It evaluates the **production** voice-model presence predicates in a fresh
//! process whose environment is configured by the driver via `Command::env`,
//! and prints the result on stdout as `<available> <reported_missing>`.
//!
//! Running the predicates out of process keeps the MBT driver from mutating its
//! own process environment: `std::env::set_var` is unsafe under concurrency
//! (other threads, including native libraries, may call `getenv`), and a test
//! harness cannot reasonably serialise all such readers.

fn main() {
    let available = voicevox_cli::infrastructure::voicevox::has_available_models();
    let reported_missing =
        voicevox_cli::infrastructure::download::missing_startup_resources().contains(&"models");

    println!("{available} {reported_missing}");
}
