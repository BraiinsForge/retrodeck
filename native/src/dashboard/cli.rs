use super::policy::{self, Game, Palette};
use std::path::PathBuf;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    pub manifest: PathBuf,
    pub palette: PathBuf,
    pub credits: Option<PathBuf>,
    pub settings_icon: Option<PathBuf>,
    pub cover_directory: Option<PathBuf>,
    pub nes_emulator: Option<String>,
    pub gb_emulator: Option<String>,
    pub gba_emulator: Option<String>,
    pub zx_emulator: Option<String>,
    pub deck_game: Option<String>,
    pub doom_deck: Option<String>,
    pub tamagotchi_deck: Option<String>,
    pub chiptune_player: Option<String>,
    pub chiptune_directory: Option<PathBuf>,
    pub volume_state: Option<PathBuf>,
    pub brightness: Option<String>,
    pub brightness_max: Option<String>,
    pub brightness_state: Option<PathBuf>,
    pub keymap_state: Option<PathBuf>,
    pub terminal: Option<String>,
    pub wifi_helper: Option<String>,
    pub wifi_status: Option<PathBuf>,
    pub validate_manifest: bool,
    pub validate_palette: bool,
    pub help: bool,
    /// Unknown launcher flags retained as generic --flag value pairs.
    pub extra: Vec<(String, String)>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            manifest: PathBuf::from(policy::DEFAULT_MANIFEST),
            palette: PathBuf::from(policy::DEFAULT_PALETTE),
            credits: None,
            settings_icon: None,
            cover_directory: None,
            nes_emulator: None,
            gb_emulator: None,
            gba_emulator: None,
            zx_emulator: None,
            deck_game: None,
            doom_deck: None,
            tamagotchi_deck: None,
            chiptune_player: None,
            chiptune_directory: None,
            volume_state: None,
            brightness: None,
            brightness_max: None,
            brightness_state: None,
            keymap_state: None,
            terminal: None,
            wifi_helper: None,
            wifi_status: None,
            validate_manifest: false,
            validate_palette: false,
            help: false,
            extra: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CliError {
    Usage(String),
    Validation(String),
}
impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(s) | Self::Validation(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for CliError {}
pub const HELP: &str = "usage: deck-menu --manifest PATH --palette PATH [--FLAG VALUE ...]";
fn value<I: Iterator<Item = String>>(a: &mut I) -> Result<String, CliError> {
    a.next().ok_or_else(|| CliError::Usage(HELP.into()))
}
pub fn parse<I, S>(args: I) -> Result<Options, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut o = Options::default();
    let arguments: Vec<String> = args.into_iter().map(Into::into).collect();
    if arguments == ["--help"] {
        o.help = true;
        return Ok(o);
    }
    if arguments.len() == 2 && arguments[0] == "--validate-manifest" {
        o.validate_manifest = true;
        o.manifest = arguments[1].clone().into();
        return Ok(o);
    }
    if arguments.len() == 2 && arguments[0] == "--validate-palette" {
        o.validate_palette = true;
        o.palette = arguments[1].clone().into();
        return Ok(o);
    }
    if arguments.len() % 2 != 0 {
        return Err(CliError::Usage(HELP.into()));
    }
    let mut seen = std::collections::HashSet::new();
    let mut a = arguments.into_iter();
    while let Some(x) = a.next() {
        if !x.starts_with("--") || x.len() <= 2 {
            return Err(CliError::Usage(HELP.into()));
        }
        if !seen.insert(x.clone()) {
            value(&mut a)?;
            continue;
        }
        match x.as_str() {
            "--manifest" => o.manifest = value(&mut a)?.into(),
            "--palette" => o.palette = value(&mut a)?.into(),
            "--credits" => o.credits = Some(value(&mut a)?.into()),
            "--settings-icon" => o.settings_icon = Some(value(&mut a)?.into()),
            "--cover-directory" => o.cover_directory = Some(value(&mut a)?.into()),
            "--nes-emulator" => o.nes_emulator = Some(value(&mut a)?),
            "--gb-emulator" => o.gb_emulator = Some(value(&mut a)?),
            "--gba-emulator" => o.gba_emulator = Some(value(&mut a)?),
            "--zx-emulator" => o.zx_emulator = Some(value(&mut a)?),
            "--deck-game" => o.deck_game = Some(value(&mut a)?),
            "--doom-deck" => o.doom_deck = Some(value(&mut a)?),
            "--tamagotchi-deck" => o.tamagotchi_deck = Some(value(&mut a)?),
            "--chiptune-player" => o.chiptune_player = Some(value(&mut a)?),
            "--chiptune-directory" => o.chiptune_directory = Some(value(&mut a)?.into()),
            "--volume-state" => o.volume_state = Some(value(&mut a)?.into()),
            "--brightness" => o.brightness = Some(value(&mut a)?),
            "--brightness-max" => o.brightness_max = Some(value(&mut a)?),
            "--brightness-state" => o.brightness_state = Some(value(&mut a)?.into()),
            "--keymap-state" => o.keymap_state = Some(value(&mut a)?.into()),
            "--terminal" => o.terminal = Some(value(&mut a)?),
            "--wifi-helper" => o.wifi_helper = Some(value(&mut a)?),
            "--wifi-status" => o.wifi_status = Some(value(&mut a)?.into()),
            _ if x.starts_with("--") => {
                let flag = x[2..].to_owned();
                if flag.is_empty() {
                    return Err(CliError::Usage(HELP.into()));
                }
                o.extra.push((flag, value(&mut a)?));
            }
            _ => return Err(CliError::Usage(HELP.into())),
        }
    }
    if !seen.contains("--manifest") || !seen.contains("--palette") {
        return Err(CliError::Usage(HELP.into()));
    }
    Ok(o)
}
impl Options {
    /// Convert launcher executable flags into the policy's typed overrides.
    pub fn launch_config(&self) -> policy::LaunchConfig {
        policy::LaunchConfig {
            nes_emulator: self.nes_emulator.clone(),
            gb_emulator: self.gb_emulator.clone(),
            gba_emulator: self.gba_emulator.clone(),
            zx_emulator: self.zx_emulator.clone(),
            deck_game: self.deck_game.clone(),
            doom_deck: self.doom_deck.clone(),
            tamagotchi_deck: self.tamagotchi_deck.clone(),
            chiptune_player: self.chiptune_player.clone(),
            terminal: self.terminal.clone(),
            reboot: None,
            doom_state_directory: None,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum Validation {
    Help,
    Run { games: Vec<Game>, palette: Palette },
    Manifest(usize),
    Palette,
}
pub fn validate(o: &Options) -> Result<Validation, CliError> {
    if o.help {
        return Ok(Validation::Help);
    }
    if o.validate_manifest {
        return policy::load_games(&o.manifest)
            .map(|g| Validation::Manifest(g.len()))
            .map_err(CliError::Validation);
    }
    if o.validate_palette {
        return policy::load_palette(&o.palette)
            .map(|_| Validation::Palette)
            .map_err(CliError::Validation);
    }
    let (g, p, _) =
        policy::load_bootstrap(&o.manifest, &o.palette).map_err(CliError::Validation)?;
    Ok(Validation::Run {
        games: g,
        palette: p,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_explicit_bootstrap_and_keeps_first_duplicate() {
        assert!(parse(Vec::<String>::new()).is_err());
        let options = parse([
            "--manifest",
            "/first",
            "--manifest",
            "/second",
            "--palette",
            "/palette",
            "--future-flag",
            "value",
        ])
        .unwrap();
        assert_eq!(options.manifest, PathBuf::from("/first"));
        assert_eq!(options.extra, vec![("future-flag".into(), "value".into())]);
        assert!(parse(["--validate-manifest", "/path", "--help"]).is_err());
    }
    #[test]
    fn unknown_is_usage() {
        assert!(matches!(parse(["--nope"]), Err(CliError::Usage(_))));
    }
    #[test]
    fn help() {
        assert_eq!(
            validate(&parse(["--help"]).unwrap()).unwrap(),
            Validation::Help
        );
    }
}
