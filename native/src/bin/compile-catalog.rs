use std::path::PathBuf;

fn main() {
    let arguments: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if arguments.len() != 4 {
        eprintln!("usage: compile-catalog SOURCE GAMES-OUTPUT PALETTE-OUTPUT PALETTE-OVERRIDE");
        std::process::exit(1);
    }
    match retrodeck_native::catalog::compile_catalog(
        &arguments[0],
        &arguments[1],
        &arguments[2],
        &arguments[3],
    ) {
        Ok((games, roles)) => println!("catalog: wrote {games} games and {roles} palette roles"),
        Err(error) => {
            eprintln!("catalog: {error}");
            std::process::exit(1);
        }
    }
}
