//! Console emulator frontend: one binary per statically linked libretro core.
//! The flake selects the core at build time through RETRO_DECK_CORE.

use std::process::ExitCode;

mod host;

fn main() -> ExitCode {
    let configuration = match option_env!("RETRO_DECK_CORE") {
        Some("nes") => &host::NES_CONFIG,
        Some("gb") => &host::GB_CONFIG,
        Some("zx") => &host::ZX_CONFIG,
        Some("gba") => &host::GBA_CONFIG,
        _ => {
            eprintln!("retro-host: RETRO_DECK_CORE must be nes, gb, zx, or gba at build time");
            return ExitCode::from(2);
        }
    };
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(host::run_host(configuration, &arguments))
}
