//! Shared catalog vocabulary. Each reader keeps its own input validation.

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
    pub const fn rom_suffix(self) -> &'static str {
        match self {
            Self::Nes => ".nes",
            Self::Gb => ".gb",
            Self::Gbc => ".gbc",
            Self::Gba => ".gba",
            Self::Zx => ".tap",
            Self::Deck => "",
        }
    }
}

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

pub const PALETTE_ROLES: &[&str] = &palette_roles();
const fn palette_roles() -> [&'static str; DEFAULT_PALETTE_VALUES.len()] {
    let mut roles = [""; DEFAULT_PALETTE_VALUES.len()];
    let mut index = 0;
    while index < roles.len() {
        roles[index] = DEFAULT_PALETTE_VALUES[index].0;
        index += 1;
    }
    roles
}
