use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments = std::env::args_os().skip(1).map(|argument| {
        argument.into_string().unwrap_or_else(|_| {
            eprintln!("deck-menu: arguments must be valid UTF-8");
            std::process::exit(2);
        })
    });
    ExitCode::from(retrodeck_native::dashboard::run(arguments) as u8)
}
