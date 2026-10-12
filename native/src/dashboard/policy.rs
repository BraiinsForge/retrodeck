use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const DEFAULT_MANIFEST: &str = "/mnt/data/nes-deck/menu/catalog.tsv";
pub const DEFAULT_PALETTE: &str = "/mnt/data/nes-deck/menu/palette.tsv";
pub const MAX_MANIFEST_BYTES: usize = 65_536;
pub const MAX_PALETTE_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum System {
    Nes,
    Gb,
    Gbc,
    Gba,
    Zx,
    Deck,
}
impl System {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "nes" => Self::Nes,
            "gb" => Self::Gb,
            "gbc" => Self::Gbc,
            "gba" => Self::Gba,
            "zx" => Self::Zx,
            "deck" => Self::Deck,
            _ => return None,
        })
    }
    pub const fn label(self) -> &'static str {
        match self {
            Self::Nes => "NES",
            Self::Gb => "GAME BOY",
            Self::Gbc => "GBC",
            Self::Gba => "GBA",
            Self::Zx => "ZX SPECTRUM",
            Self::Deck => "DECK",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub system: System,
    pub rom: String,
    pub cover: Option<PathBuf>,
    pub color: u32,
    pub terminal_mode: Option<String>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Palette {
    pub colors: Vec<(String, u32)>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Timings {
    pub child_touch_exit_ms: u64,
    pub child_term_grace_ms: u64,
    pub reboot_confirm_ms: u64,
    pub controller_burst_window_ms: u64,
    pub controller_quiet_reset_ms: u64,
    pub controller_scan_ms: u64,
    pub touch_reconnect_ms: u64,
    pub main_poll_ms: u64,
    pub animated_poll_ms: u64,
    pub network_refresh_ms: u64,
    pub console_mirror_ms: u64,
}
impl Default for Timings {
    fn default() -> Self {
        Self {
            child_touch_exit_ms: 2000,
            child_term_grace_ms: 4000,
            reboot_confirm_ms: 4000,
            controller_burst_window_ms: 1000,
            controller_quiet_reset_ms: 1000,
            controller_scan_ms: 1000,
            touch_reconnect_ms: 1000,
            main_poll_ms: 250,
            animated_poll_ms: 66,
            network_refresh_ms: 2000,
            console_mirror_ms: 100,
        }
    }
}
pub const VOLUME_DEFAULT: u8 = 42;
pub const VOLUME_STEP: u8 = 5;
pub const BRIGHTNESS_MINIMUM: u8 = 10;
pub const BRIGHTNESS_STEP: u8 = 10;
pub const CONTROLLER_BURST_LIMIT: usize = 12;
pub const DEFAULT_PALETTE_VALUES: &[(&str, u32)] = &[
    ("background", 0),
    ("text-dark", 0x121212),
    ("field", 0x121212),
    ("surface", 0x1c1c1c),
    ("inactive-border", 0x5f5f5f),
    ("control-border", 0x6c6c6c),
    ("footer", 0xbcbcbc),
    ("inactive-text", 0xdadada),
    ("text", 0xeeeeee),
    ("white", 0xffffff),
    ("title", 0xffffaf),
    ("volume-off", 0xaf8787),
    ("volume-on", 0x87af87),
    ("selected", 0xecb6e7),
    ("wifi-active", 0x5f87af),
    ("wifi-focus", 0x87afff),
    ("wifi-active-border", 0xafafff),
    ("field-label", 0xafafaf),
    ("accent", 0xfe6c27),
    ("active", 0x503311),
    ("control-surface", 0x303030),
    ("muted", 0x949494),
];
fn builtin(id: &str, title: &str, rom: &str, color: u32, mode: Option<&str>) -> Game {
    Game {
        id: id.into(),
        title: title.into(),
        system: System::Deck,
        rom: rom.into(),
        color,
        cover: None,
        terminal_mode: mode.map(str::to_owned),
    }
}
pub fn builtins() -> Vec<Game> {
    vec![
        builtin(
            "lua-repl",
            "LUA REPL",
            "/mnt/data/nes-deck/terminal/retro-terminal",
            0x5f87ff,
            Some("lua"),
        ),
        builtin(
            "lisp-repl",
            "LISP REPL",
            "/mnt/data/nes-deck/terminal/retro-terminal",
            0xafd75f,
            Some("lisp"),
        ),
        builtin(
            "python-repl",
            "PYTHON REPL",
            "/mnt/data/nes-deck/terminal/retro-terminal",
            0xffd700,
            Some("python"),
        ),
        builtin(
            "scheme-repl",
            "SCHEME REPL",
            "/mnt/data/nes-deck/terminal/retro-terminal",
            0x87d787,
            Some("scheme"),
        ),
        builtin(
            "chiptunes",
            "CHIPTUNES",
            "/mnt/data/chiptunes",
            0xff8700,
            None,
        ),
        builtin(
            "terminal",
            "TERMINAL",
            "/mnt/data/nes-deck/terminal/retro-terminal",
            0x5f87af,
            Some("shell"),
        ),
        builtin(
            "node-mode",
            "NODE MODE",
            "/mnt/data/nes-deck/menu/node-mode",
            0xfe6c27,
            None,
        ),
        builtin("reboot", "REBOOT", "/sbin/reboot", 0xd75f5f, None),
    ]
}
fn bounded(path: &Path, max: usize, what: &str) -> Result<String, String> {
    let bytes = crate::regular_file::read_regular(path, 1, max as u64, what)?
        .ok_or_else(|| format!("Cannot read {what} {}: not found", path.display()))?;
    String::from_utf8(bytes).map_err(|_| format!("{what} is not UTF-8"))
}
fn color(s: &str, c: &str) -> Result<u32, String> {
    if s.len() != 7 || !s.starts_with('#') || !s[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("Invalid #RRGGBB color in {c}"));
    }
    u32::from_str_radix(&s[1..], 16).map_err(|_| format!("Invalid #RRGGBB color in {c}"))
}
fn valid_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 48
        && s.bytes().enumerate().all(|(i, b)| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || (b == b'-' && i > 0 && i + 1 < s.len())
        })
}
fn display(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_ascii_control() {
                None
            } else if c.is_ascii() {
                Some(c)
            } else {
                Some('?')
            }
        })
        .take(64)
        .collect()
}
pub fn load_games(path: &Path) -> Result<Vec<Game>, String> {
    let t = bounded(path, MAX_MANIFEST_BYTES, "Manifest")?;
    let (mut out, mut ids, mut roms) = (Vec::new(), HashSet::new(), HashSet::new());
    let mut data = false;
    for (ln, r) in t.lines().enumerate() {
        let n = ln + 1;
        if r.len() > 4096 {
            return Err(format!("Manifest line {n} exceeds 4096 bytes"));
        }
        let l = r.strip_suffix('\r').unwrap_or(r);
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<_> = l.split('\t').collect();
        if !data
            && f.len() == 5
            && f[..4] == ["id", "title", "system", "rom"]
            && (f[4] == "color" || f[4] == "#RRGGBB")
        {
            data = true;
            continue;
        }
        data = true;
        if f.len() != 5 {
            return Err(format!("Manifest line {n} must have 5 fields"));
        }
        if !valid_id(f[0]) {
            return Err(format!("Invalid id on manifest line {n}"));
        }
        let title = display(f[1]);
        if title.is_empty() || f[1].starts_with(' ') || f[1].ends_with(' ') {
            return Err(format!("Invalid title on manifest line {n}"));
        }
        if f[3].is_empty() || f[3].len() > 4095 || !f[3].starts_with('/') || f[3].ends_with(' ') {
            return Err(format!("Invalid ROM path on manifest line {n}"));
        }
        let system =
            System::parse(f[2]).ok_or_else(|| format!("Invalid system on manifest line {n}"))?;
        if !ids.insert(f[0]) {
            return Err(format!("Duplicate id on manifest line {n}"));
        }
        if !roms.insert(f[3]) {
            return Err(format!("Duplicate ROM on manifest line {n}"));
        }
        out.push(Game {
            id: f[0].into(),
            title,
            system,
            rom: f[3].into(),
            color: color(f[4], &format!("manifest line {n}"))?,
            terminal_mode: None,
            cover: None,
        });
        if out.len() > 64 {
            return Err("Manifest contains more than 64 games".into());
        }
    }
    if out.is_empty() {
        Err("Manifest contains no games".into())
    } else {
        Ok(out)
    }
}
pub fn load_palette(path: &Path) -> Result<Palette, String> {
    let t = bounded(path, MAX_PALETTE_BYTES, "Palette")?;
    let mut got = std::collections::HashMap::new();
    let mut icon = false;
    for (ln, r) in t.lines().enumerate() {
        let n = ln + 1;
        let l = r.strip_suffix('\r').unwrap_or(r);
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let f: Vec<_> = l.split('\t').collect();
        if f.len() != 2 {
            return Err(format!("Palette line {n} must have 2 fields"));
        }
        if f[0] == "settings-icon" {
            if icon
                || f[1].is_empty()
                || f[1].len() > 64
                || !f[1]
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err(format!("Invalid legacy settings icon on palette line {n}"));
            }
            icon = true
        } else {
            if !DEFAULT_PALETTE_VALUES.iter().any(|(k, _)| *k == f[0]) {
                return Err(format!("Unknown palette role on line {n}"));
            }
            if got
                .insert(f[0], color(f[1], &format!("palette line {n}"))?)
                .is_some()
            {
                return Err(format!("Duplicate palette role on line {n}"));
            }
        }
    }
    let mut colors = Vec::new();
    for (k, _) in DEFAULT_PALETTE_VALUES {
        let v = *got
            .get(k)
            .ok_or_else(|| format!("Palette is missing role {k}"))?;
        colors.push(((*k).into(), v));
    }
    Ok(Palette { colors })
}
pub fn load_bootstrap(m: &Path, p: &Path) -> Result<(Vec<Game>, Palette, bool), String> {
    let mut g = load_games(m)?;
    g.extend(builtins());
    match load_palette(p) {
        Ok(x) => Ok((g, x, true)),
        Err(e) => {
            eprintln!("retrodeck: {e}; using startup dashboard palette");
            Ok((
                g,
                Palette {
                    colors: DEFAULT_PALETTE_VALUES
                        .iter()
                        .map(|(k, v)| (k.to_string(), *v))
                        .collect(),
                },
                false,
            ))
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchPlan {
    pub executable: String,
    pub arguments: Vec<String>,
    pub environment: Vec<(String, String)>,
    pub label: String,
    pub kind: LaunchKind,
    pub touch_supervision: bool,
    pub mirror_console: bool,
    pub volume: u8,
    pub keymap: String,
    pub wayland: bool,
    pub volume_state: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchKind {
    Game,
    Terminal,
    Reboot,
}

/// Runtime executable overrides supplied by the launcher CLI. None selects the
/// default production path, not an empty executable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LaunchConfig {
    pub nes_emulator: Option<String>,
    pub gb_emulator: Option<String>,
    pub gba_emulator: Option<String>,
    pub zx_emulator: Option<String>,
    pub deck_game: Option<String>,
    pub doom_deck: Option<String>,
    pub tamagotchi_deck: Option<String>,
    pub chiptune_player: Option<String>,
    pub terminal: Option<String>,
    pub reboot: Option<String>,
    pub doom_state_directory: Option<String>,
}

impl LaunchConfig {
    fn executable<'a>(&'a self, route: &str, default: &'a str) -> &'a str {
        match route {
            "nes" => self.nes_emulator.as_deref(),
            "gb" => self.gb_emulator.as_deref(),
            "gba" => self.gba_emulator.as_deref(),
            "zx" => self.zx_emulator.as_deref(),
            "deck" => self.deck_game.as_deref(),
            "doom" => self.doom_deck.as_deref(),
            "tamagotchi" => self.tamagotchi_deck.as_deref(),
            "chiptunes" => self.chiptune_player.as_deref(),
            "terminal" => self.terminal.as_deref(),
            "reboot" => self.reboot.as_deref(),
            _ => None,
        }
        .unwrap_or(default)
    }
}

fn wad_application(g: &Game) -> bool {
    g.system == System::Deck
        && g.rom.len() > 4
        && g.rom[..g.rom.len() - 4].len() > 0
        && g.rom[g.rom.len() - 4..].eq_ignore_ascii_case(".wad")
}

/// Construct the exact child process contract used by the dashboard policy.
pub fn launch_plan(
    g: &Game,
    volume: u8,
    keymap: &str,
    wayland: bool,
    volume_state: Option<&Path>,
    config: &LaunchConfig,
) -> LaunchPlan {
    assert!(volume <= 100, "volume must be between 0 and 100");
    let terminal_mode = g.terminal_mode.as_deref();
    if let Some(mode) = terminal_mode {
        let exe = config.executable("terminal", "/mnt/data/nes-deck/terminal/retro-terminal");
        return LaunchPlan {
            executable: exe.into(),
            arguments: vec![mode.into()],
            environment: vec![("RETRO_DECK_KEYMAP".into(), keymap.into())],
            label: if mode == "shell" {
                "terminal".into()
            } else {
                format!("{mode} REPL")
            },
            kind: LaunchKind::Terminal,
            touch_supervision: true,
            mirror_console: true,
            volume,
            keymap: keymap.into(),
            wayland,
            volume_state: volume_state.map(Path::to_path_buf),
        };
    }
    if g.system == System::Deck && g.id == "reboot" {
        return LaunchPlan {
            executable: config.executable("reboot", &g.rom).into(),
            arguments: vec![],
            environment: vec![],
            label: "reboot".into(),
            kind: LaunchKind::Reboot,
            touch_supervision: true,
            mirror_console: false,
            volume,
            keymap: keymap.into(),
            wayland,
            volume_state: volume_state.map(Path::to_path_buf),
        };
    }
    let route = match g.system {
        System::Nes => "nes",
        System::Gb | System::Gbc => "gb",
        System::Gba => "gba",
        System::Zx => "zx",
        System::Deck if g.id == "node-mode" => "node-mode",
        System::Deck if g.id == "tamagotchi" => "tamagotchi",
        System::Deck if g.id == "chiptunes" => "chiptunes",
        System::Deck if wad_application(g) => "doom",
        System::Deck => "deck",
    };
    let default = match route {
        "nes" => "/mnt/data/nes-deck/nes-deck",
        "gb" => "/mnt/data/nes-deck/gb-deck",
        "gba" => "/mnt/data/nes-deck/gba-deck",
        "zx" => "/mnt/data/nes-deck/zx-deck",
        "node-mode" => "/mnt/data/nes-deck/menu/node-mode",
        "tamagotchi" => "/mnt/data/nes-deck/tamagotchi-deck",
        "chiptunes" => "/mnt/data/nes-deck/chiptune-deck",
        "doom" => "/mnt/data/nes-deck/doom-deck",
        _ => "/mnt/data/nes-deck/ten-seconds-deck",
    };
    let mut environment = vec![("RETRO_DECK_VOLUME_PERCENT".into(), volume.to_string())];
    let has_rom_arg =
        g.system != System::Deck || matches!(route, "chiptunes" | "doom" | "tamagotchi");
    if g.system != System::Deck || matches!(route, "doom" | "tamagotchi") {
        environment.push(("RETRO_DECK_EXIT_HINT".into(), "1".into()));
    }
    if route == "doom" {
        environment.push((
            "RETRO_DECK_DOOM_STATE".into(),
            config
                .doom_state_directory
                .as_deref()
                .unwrap_or("/mnt/data/doom")
                .into(),
        ));
    }
    if wayland {
        environment.push(("RETRO_DECK_PRESENTATION".into(), "widget".into()));
    }
    if let Some(state) = volume_state.filter(|p| !p.as_os_str().is_empty()) {
        environment.push((
            "RETRO_DECK_VOLUME_STATE".into(),
            state.to_string_lossy().into_owned(),
        ));
    }
    LaunchPlan {
        executable: config.executable(route, default).into(),
        arguments: if has_rom_arg {
            vec![g.rom.clone()]
        } else {
            vec![]
        },
        environment,
        label: g.id.clone(),
        kind: LaunchKind::Game,
        touch_supervision: wayland
            || g.system != System::Deck
            || matches!(route, "doom" | "tamagotchi"),
        mirror_console: false,
        volume,
        keymap: keymap.into(),
        wayland,
        volume_state: volume_state.map(Path::to_path_buf),
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControllerGuard {
    pub edge_times: Vec<u64>,
    pub suspended: bool,
    pub last_edge_at: Option<u64>,
}
impl ControllerGuard {
    pub fn new() -> Self {
        Self {
            edge_times: vec![],
            suspended: false,
            last_edge_at: None,
        }
    }
    pub fn accept_edge(&mut self, now: u64) -> bool {
        self.last_edge_at = Some(now);
        if self.suspended {
            return false;
        }
        self.edge_times.retain(|t| now.saturating_sub(*t) < 1000);
        self.edge_times.push(now);
        if self.edge_times.len() <= 12 {
            true
        } else {
            self.suspended = true;
            false
        }
    }
    pub fn recover_if_quiet(&mut self, now: u64) -> bool {
        if self.suspended
            && self
                .last_edge_at
                .is_some_and(|t| now.saturating_sub(t) >= 1000)
        {
            *self = Self::new();
            true
        } else {
            false
        }
    }
}
pub fn control_priority(back: bool, physical_a: bool, touch: bool) -> Option<&'static str> {
    if back {
        Some("back")
    } else if physical_a {
        Some("confirm")
    } else if touch {
        Some("touch")
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn game(id: &str, system: System, rom: &str) -> Game {
        Game {
            id: id.into(),
            title: id.into(),
            system,
            rom: rom.into(),
            cover: None,
            color: 0,
            terminal_mode: None,
        }
    }

    #[test]
    fn builtins_end_in_reboot() {
        assert_eq!(builtins().last().unwrap().id, "reboot");
    }

    #[test]
    fn launch_contract_covers_all_routes_and_overrides() {
        let mut c = LaunchConfig::default();
        c.nes_emulator = Some("nes-x".into());
        c.gb_emulator = Some("gb-x".into());
        c.gba_emulator = Some("gba-x".into());
        c.zx_emulator = Some("zx-x".into());
        c.deck_game = Some("deck-x".into());
        c.doom_deck = Some("doom-x".into());
        c.tamagotchi_deck = Some("tama-x".into());
        c.chiptune_player = Some("chip-x".into());
        let p = |g: Game| launch_plan(&g, 37, "de", true, Some(Path::new("/state/volume")), &c);
        let nes = p(game("n", System::Nes, "/rom/n.nes"));
        assert_eq!(
            (&nes.executable, &nes.arguments),
            (&"nes-x".into(), &vec!["/rom/n.nes".into()])
        );
        let gbc = p(game("c", System::Gbc, "/rom/c.gbc"));
        assert_eq!(gbc.executable, "gb-x");
        let gba = p(game("a", System::Gba, "/rom/a.gba"));
        assert_eq!(gba.executable, "gba-x");
        assert_eq!(p(game("z", System::Zx, "/rom/z.tap")).executable, "zx-x");
        let deck = p(game("d", System::Deck, "/rom/d"));
        assert_eq!(deck.executable, "deck-x");
        assert!(deck.arguments.is_empty());
        let node = p(game("node-mode", System::Deck, "/node"));
        assert_eq!(node.executable, "/mnt/data/nes-deck/menu/node-mode");
        let tama = p(game("tamagotchi", System::Deck, "/games/tama"));
        assert_eq!(tama.arguments, vec!["/games/tama"]);
        assert!(
            tama.environment
                .iter()
                .any(|x| x == &("RETRO_DECK_EXIT_HINT".into(), "1".into()))
        );
        let chip = p(game("chiptunes", System::Deck, "/music"));
        assert_eq!(chip.executable, "chip-x");
        assert_eq!(chip.arguments, vec!["/music"]);
        let doom = p(game("wad", System::Deck, "/games/doom.wad"));
        assert_eq!(doom.executable, "doom-x");
        assert!(
            doom.environment
                .iter()
                .any(|x| x.0 == "RETRO_DECK_DOOM_STATE")
        );
        let mut repl = game("lisp-repl", System::Deck, "/terminal");
        repl.terminal_mode = Some("lisp".into());
        let repl = p(repl);
        assert_eq!(repl.kind, LaunchKind::Terminal);
        assert_eq!(repl.arguments, vec!["lisp"]);
        assert_eq!(
            repl.environment,
            vec![("RETRO_DECK_KEYMAP".into(), "de".into())]
        );
        let reboot = p(game("reboot", System::Deck, "/sbin/reboot"));
        assert_eq!(reboot.kind, LaunchKind::Reboot);
        assert!(reboot.arguments.is_empty() && reboot.environment.is_empty());
        assert_eq!(reboot.volume, 37);
        assert!(reboot.wayland && reboot.volume_state == Some(PathBuf::from("/state/volume")));
    }

    #[test]
    fn guard() {
        let mut g = ControllerGuard::new();
        for i in 0..12 {
            assert!(g.accept_edge(i));
        }
        assert!(!g.accept_edge(12));
        assert!(g.recover_if_quiet(1012));
    }

    #[test]
    fn priority() {
        assert_eq!(control_priority(true, true, true), Some("back"));
    }
}
