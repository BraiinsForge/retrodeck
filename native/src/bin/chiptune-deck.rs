use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() != 1 {
        eprintln!("usage: chiptune-deck CHIPTUNE_DIRECTORY");
        return ExitCode::from(2);
    }
    match retrodeck_native::chiptune_ui::run(&PathBuf::from(&arguments[0])) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("chiptune-deck: {error}");
            ExitCode::FAILURE
        }
    }
}
