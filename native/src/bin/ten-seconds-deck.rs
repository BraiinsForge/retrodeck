use std::process::ExitCode;

fn main() -> ExitCode {
    if std::env::args_os().len() != 1 {
        eprintln!("usage: ten-seconds-deck");
        return ExitCode::from(2);
    }
    match retrodeck_native::ten_seconds::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ten-seconds-deck: {error}");
            ExitCode::FAILURE
        }
    }
}
