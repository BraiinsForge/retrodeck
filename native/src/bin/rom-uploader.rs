use retrodeck_native::uploader::{Config, PasswordConfig, serve, verify_password_stdin};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

const USAGE: &str = "usage: rom-uploader [--data-root PATH] [--assets PATH] [--address IPv4] [--port PORT] [--restart-program PATH | --no-restart] [--ready-file PATH]\n       rom-uploader --check-uploader-password-config PATH\n       rom-uploader --verify-uploader-password PATH";

async fn shutdown() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install SIGTERM handler");
    tokio::select! {_ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {}}
}
async fn run() -> Result<u8, String> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|a| {
        a == "--check-uploader-password-config" || a == "--verify-uploader-password"
    }) {
        if args.len() != 2 {
            return Ok(2);
        }
        let path = Path::new(&args[1]);
        return if args[0] == "--check-uploader-password-config" {
            PasswordConfig::read(path).map(|_| 0)
        } else {
            verify_password_stdin(path)
        };
    }
    let mut config = Config::default();
    let mut ready_file: Option<PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].to_str().ok_or("argument is not UTF-8")?;
        if flag == "--no-restart" {
            config.restart_command = None;
            index += 1;
            continue;
        }
        if flag == "--help" && args.len() == 1 {
            println!("{USAGE}");
            return Ok(0);
        }
        if index + 1 >= args.len() {
            return Ok(2);
        }
        let value = &args[index + 1];
        match flag {
            "--data-root" => config.data_root = value.into(),
            "--assets" => config.asset_directory = value.into(),
            "--address" => {
                config.address = value
                    .to_str()
                    .and_then(|v| v.parse().ok())
                    .ok_or("invalid IPv4 address")?
            }
            "--port" => {
                config.port = value
                    .to_str()
                    .and_then(|v| v.parse().ok())
                    .ok_or("invalid port")?
            }
            "--restart-program" => {
                config.restart_command = Some(vec![
                    value.to_str().ok_or("restart program is not UTF-8")?.into(),
                ])
            }
            "--ready-file" => ready_file = Some(value.into()),
            _ => return Ok(2),
        }
        index += 2;
    }
    serve(
        config,
        move |address| {
            println!("rom-uploader: listening on {address}");
            if let Some(path) = ready_file {
                if let Err(error) = std::fs::write(path, format!("{}\n", address.port())) {
                    eprintln!("rom-uploader: cannot write ready file: {error}");
                }
            }
        },
        shutdown(),
    )
    .await?;
    Ok(0)
}
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> ExitCode {
    match run().await {
        Ok(2) => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("rom-uploader: {error}");
            ExitCode::FAILURE
        }
    }
}
