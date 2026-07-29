//! DOOM frontend: the fbDOOM engine linked in as a static archive, with the
//! Deck's display, controllers, and audio supplied by the Rust host.

use std::process::ExitCode;

mod host;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(host::run_host(&arguments))
}
