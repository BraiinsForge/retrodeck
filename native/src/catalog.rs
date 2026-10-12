//! Data-only catalog reader and TSV compiler. No Lisp runtime is required.
//!
//! This deliberately implements a bounded subset of the Common Lisp reader:
//! proper lists, keywords, decimal integers, strings, and line/block comments.
//! In strings a backslash quotes the next character, including `n` literally.
//! Reader dispatch, quote forms, dotted lists, and symbol escapes are rejected.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAXIMUM_CATALOG_BYTES: usize = 65_536;
const MAXIMUM_DEPTH: usize = 32;
const MAXIMUM_NODES: usize = 4096;
const MAXIMUM_GAMES: usize = 64;
pub const PALETTE_ROLES: &[&str] = &[
    "background",
    "text-dark",
    "field",
    "surface",
    "inactive-border",
    "control-border",
    "footer",
    "inactive-text",
    "text",
    "white",
    "title",
    "volume-off",
    "volume-on",
    "selected",
    "wifi-active",
    "wifi-focus",
    "wifi-active-border",
    "field-label",
    "accent",
    "active",
    "control-surface",
    "muted",
];

#[derive(Debug)]
pub struct CatalogError(String);
impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Catalog error: {}", self.0)
    }
}
impl std::error::Error for CatalogError {}
impl From<std::io::Error> for CatalogError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}
type Result<T> = std::result::Result<T, CatalogError>;
fn invalid(message: impl Into<String>) -> CatalogError {
    CatalogError(message.into())
}

#[derive(Debug, PartialEq)]
enum Form {
    List(Vec<Form>),
    Keyword(String),
    String(String),
    Integer(i64),
}
struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
    nodes: usize,
}
impl Reader<'_> {
    fn starts(&self, token: &[u8]) -> bool {
        self.bytes[self.cursor..].starts_with(token)
    }
    fn skip_space(&mut self) -> Result<()> {
        loop {
            while self
                .bytes
                .get(self.cursor)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.cursor += 1;
            }
            if self.starts(b";") {
                while self.bytes.get(self.cursor).is_some_and(|b| *b != b'\n') {
                    self.cursor += 1;
                }
            } else if self.starts(b"#|") {
                self.cursor += 2;
                let mut depth = 1;
                while depth > 0 {
                    if self.cursor == self.bytes.len() {
                        return Err(invalid("unterminated block comment"));
                    }
                    if self.starts(b"#|") {
                        depth += 1;
                        if depth > MAXIMUM_DEPTH {
                            return Err(invalid("comment nesting limit exceeded"));
                        }
                        self.cursor += 2;
                    } else if self.starts(b"|#") {
                        depth -= 1;
                        self.cursor += 2;
                    } else {
                        self.cursor += 1;
                    }
                }
            } else {
                return Ok(());
            }
        }
    }
    fn form(&mut self, depth: usize) -> Result<Form> {
        self.skip_space()?;
        if depth > MAXIMUM_DEPTH {
            return Err(invalid("list nesting limit exceeded"));
        }
        self.nodes += 1;
        if self.nodes > MAXIMUM_NODES {
            return Err(invalid("input node limit exceeded"));
        }
        let byte = *self
            .bytes
            .get(self.cursor)
            .ok_or_else(|| invalid("unexpected end of input"))?;
        self.cursor += 1;
        match byte {
            b'(' => {
                let mut values = Vec::new();
                loop {
                    self.skip_space()?;
                    if self.bytes.get(self.cursor) == Some(&b')') {
                        self.cursor += 1;
                        break;
                    }
                    values.push(self.form(depth + 1)?);
                }
                Ok(Form::List(values))
            }
            b'"' => {
                let mut value = Vec::new();
                loop {
                    let byte = *self
                        .bytes
                        .get(self.cursor)
                        .ok_or_else(|| invalid("unterminated string"))?;
                    self.cursor += 1;
                    match byte {
                        b'"' => break,
                        b'\\' => {
                            value.push(
                                *self
                                    .bytes
                                    .get(self.cursor)
                                    .ok_or_else(|| invalid("unterminated string escape"))?,
                            );
                            self.cursor += 1;
                        }
                        _ => value.push(byte),
                    }
                }
                Ok(Form::String(
                    String::from_utf8(value).map_err(|_| invalid("invalid UTF-8 string"))?,
                ))
            }
            b')' | b'#' | b'\'' | b'`' | b',' | b'|' | b'\\' => {
                Err(invalid("unsupported reader syntax"))
            }
            _ => {
                let start = self.cursor - 1;
                while self
                    .bytes
                    .get(self.cursor)
                    .is_some_and(|b| !b.is_ascii_whitespace() && !b"();\"".contains(b))
                {
                    self.cursor += 1;
                }
                let token = std::str::from_utf8(&self.bytes[start..self.cursor])
                    .map_err(|_| invalid("invalid token"))?;
                if let Some(keyword) = token.strip_prefix(':') {
                    if keyword.is_empty()
                        || !keyword
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        return Err(invalid("invalid keyword"));
                    }
                    Ok(Form::Keyword(keyword.to_ascii_lowercase()))
                } else if token.eq_ignore_ascii_case("nil") {
                    Ok(Form::List(Vec::new()))
                } else {
                    // Common Lisp decimal integer tokens may end in a decimal point.
                    let integer = token.strip_suffix('.').unwrap_or(token);
                    let digits = integer.strip_prefix(['+', '-']).unwrap_or(integer);
                    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(invalid("unsupported symbol or number"));
                    }
                    Ok(Form::Integer(
                        integer
                            .parse()
                            .map_err(|_| invalid("integer out of range"))?,
                    ))
                }
            }
        }
    }
}
fn read_form(source: &str) -> Result<Form> {
    if source.len() > MAXIMUM_CATALOG_BYTES {
        return Err(invalid("input exceeds the 65536-byte limit"));
    }
    let mut reader = Reader {
        bytes: source.as_bytes(),
        cursor: 0,
        nodes: 0,
    };
    let form = reader.form(0)?;
    reader.skip_space()?;
    if reader.cursor != source.len() {
        return Err(invalid("more than one top-level form"));
    }
    Ok(form)
}
fn list<'a>(form: &'a Form, limit: usize, context: &str) -> Result<&'a [Form]> {
    match form {
        Form::List(values) if !values.is_empty() && values.len() <= limit => Ok(values),
        _ => Err(invalid(format!(
            "{context} must be a nonempty proper list of at most {limit} elements"
        ))),
    }
}
fn plist<'a>(
    form: &'a Form,
    allowed: &[&str],
    required: &[&str],
    context: &str,
) -> Result<HashMap<&'a str, &'a Form>> {
    let values = list(form, 2 * allowed.len() + 2, context)?;
    if values.len() % 2 != 0 {
        return Err(invalid(format!("{context} has a key without a value")));
    }
    let mut pairs = HashMap::new();
    for pair in values.chunks_exact(2) {
        let Form::Keyword(key) = &pair[0] else {
            return Err(invalid(format!("{context} key must be a keyword")));
        };
        if !allowed.contains(&key.as_str()) {
            return Err(invalid(format!("{context} contains unknown key :{key}")));
        }
        if pairs.insert(key.as_str(), &pair[1]).is_some() {
            return Err(invalid(format!("{context} contains duplicate key :{key}")));
        }
    }
    for key in required {
        if !pairs.contains_key(key) {
            return Err(invalid(format!("{context} is missing key :{key}")));
        }
    }
    Ok(pairs)
}
fn text<'a>(form: &'a Form, maximum: usize, context: &str) -> Result<&'a str> {
    match form {
        Form::String(value)
            if !value.is_empty()
                && value.len() <= maximum
                && value.bytes().all(|b| (32..=126).contains(&b)) =>
        {
            Ok(value)
        }
        _ => Err(invalid(format!(
            "{context} must be 1 to {maximum} printable ASCII characters"
        ))),
    }
}
fn rgb(form: &Form) -> Result<String> {
    let value = text(form, 7, "color")?;
    if value.len() != 7
        || !value.starts_with('#')
        || !value[1..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(invalid("color must have the form #RRGGBB"));
    }
    Ok(value.to_ascii_uppercase())
}
fn game_color(form: &Form) -> Result<String> {
    let color = rgb(form)?;
    let value = u32::from_str_radix(&color[1..], 16).expect("validated RGB");
    let components = [(value >> 16) as u8, (value >> 8) as u8, value as u8];
    let ansi = [
        0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xC0C0C0, 0x808080,
        0xFF0000, 0x00FF00, 0xFFFF00, 0x0000FF, 0xFF00FF, 0x00FFFF, 0xFFFFFF,
    ];
    let gray = components[0];
    if !(ansi.contains(&value)
        || components
            .iter()
            .all(|v| [0, 95, 135, 175, 215, 255].contains(v))
        || (components == [gray; 3] && (8..=238).contains(&gray) && (gray - 8) % 10 == 0))
    {
        return Err(invalid("game :color must be from the xterm-256 palette"));
    }
    Ok(color)
}
fn palette(form: &Form) -> Result<Vec<(String, String)>> {
    let values = plist(form, PALETTE_ROLES, PALETTE_ROLES, "palette")?;
    PALETTE_ROLES
        .iter()
        .map(|key| Ok(((*key).to_owned(), rgb(values[key])?)))
        .collect()
}

#[derive(Debug, PartialEq)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub system: String,
    pub rom: String,
    pub color: String,
}
#[derive(Debug, PartialEq)]
pub struct Catalog {
    pub games: Vec<Game>,
    pub palette: Vec<(String, String)>,
}
fn game(form: &Form) -> Result<Game> {
    let keys = ["id", "title", "system", "rom", "color"];
    let values = plist(form, &keys, &keys, "game")?;
    let id = text(values["id"], 32, "game :id")?;
    if !id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || id.starts_with('-')
        || id.ends_with('-')
        || id.contains("--")
    {
        return Err(invalid(
            "game :id must use lowercase letters, digits, and single interior hyphens",
        ));
    }
    let title = text(values["title"], 48, "game :title")?;
    if title.trim_matches(' ') != title {
        return Err(invalid(
            "game :title must not have leading or trailing spaces",
        ));
    }
    let Form::Keyword(system) = values["system"] else {
        return Err(invalid("game :system must be a keyword"));
    };
    let suffix = match system.as_str() {
        "nes" => ".nes",
        "gb" => ".gb",
        "gbc" => ".gbc",
        "gba" => ".gba",
        "zx" => ".tap",
        "deck" => "",
        _ => return Err(invalid("unsupported game :system")),
    };
    let rom = text(values["rom"], 512, "game :rom")?;
    let prefix = if system == "deck" {
        "/mnt/data/nes-deck/games/".to_owned()
    } else {
        format!("/mnt/data/roms/{system}/")
    };
    if !rom.starts_with(&prefix)
        || !rom.ends_with(suffix)
        || !rom
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
        || rom.contains("//")
        || rom.contains("/./")
        || rom.contains("/../")
    {
        return Err(invalid(format!(
            "game :rom must be a normalized {suffix} path below {prefix}"
        )));
    }
    Ok(Game {
        id: id.to_owned(),
        title: title.to_owned(),
        system: system.to_owned(),
        rom: rom.to_owned(),
        color: game_color(values["color"])?,
    })
}

/// Validate schema 7, preserving game order and canonical palette-role order.
pub fn parse_catalog(source: &str) -> Result<Catalog> {
    let form = read_form(source)?;
    let keys = ["version", "palette", "games"];
    let values = plist(&form, &keys, &keys, "catalog")?;
    if values["version"] != &Form::Integer(7) {
        return Err(invalid("unsupported :version; expected 7"));
    }
    let palette = palette(values["palette"])?;
    let games: Vec<Game> = list(values["games"], MAXIMUM_GAMES, "catalog :games")?
        .iter()
        .map(game)
        .collect::<Result<_>>()?;
    let mut ids = HashSet::new();
    let mut roms = HashSet::new();
    for game in &games {
        if !ids.insert(&game.id) {
            return Err(invalid(format!("duplicate game :id {:?}", game.id)));
        }
        if !roms.insert(&game.rom) {
            return Err(invalid(format!("duplicate game :rom {:?}", game.rom)));
        }
    }
    Ok(Catalog { games, palette })
}
/// Read appearance schema 2 or 3. The legacy icon name is validated but not emitted.
pub fn parse_appearance_override(source: &str) -> Result<Vec<(String, String)>> {
    let form = read_form(source)?;
    let values = plist(
        &form,
        &["version", "settings-icon", "palette"],
        &["version", "palette"],
        "appearance override",
    )?;
    let version = values["version"];
    if version != &Form::Integer(2) && version != &Form::Integer(3) {
        return Err(invalid(
            "unsupported appearance override :version; expected 2 or 3",
        ));
    }
    let icon = values.get("settings-icon");
    if version == &Form::Integer(2) && icon.is_some() {
        return Err(invalid("appearance override version 2 cannot set an icon"));
    }
    if version == &Form::Integer(3) && icon.is_none() {
        return Err(invalid(
            "appearance override version 3 is missing :settings-icon",
        ));
    }
    if let Some(icon) = icon {
        let icon = text(icon, 64, "appearance override :settings-icon")?;
        if !icon
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(invalid("invalid legacy cog name"));
        }
    }
    palette(values["palette"])
}
impl Catalog {
    pub fn games_tsv(&self) -> String {
        let mut result = String::new();
        for game in &self.games {
            result.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                game.id, game.title, game.system, game.rom, game.color
            ));
        }
        result
    }
    pub fn palette_tsv(&self) -> String {
        self.palette
            .iter()
            .map(|(key, value)| format!("{key}\t{value}\n"))
            .collect()
    }
}
fn read_source(path: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((MAXIMUM_CATALOG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAXIMUM_CATALOG_BYTES {
        return Err(invalid(format!(
            "{} exceeds the 65536-byte input limit",
            path.display()
        )));
    }
    String::from_utf8(bytes).map_err(|_| invalid("input is not UTF-8"))
}
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn prepare(output: &Path, bytes: &[u8]) -> Result<Temporary> {
    let mut name = output.as_os_str().to_os_string();
    name.push(format!(
        ".tmp.{}.{}",
        std::process::id(),
        NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    let path = PathBuf::from(name);
    // Do not follow a pre-existing temporary-file symlink or truncate another process's file.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let temporary = Temporary(path);
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(temporary)
}
/// Validate both sources before writing; rename palette first, then games, as before.
/// Each output rename is atomic. The two outputs are not a filesystem transaction.
pub fn compile_catalog(
    source: &Path,
    games_output: &Path,
    palette_output: &Path,
    appearance_override: &Path,
) -> Result<(usize, usize)> {
    if games_output == palette_output {
        return Err(invalid("games and palette outputs must be distinct"));
    }
    let mut catalog = parse_catalog(&read_source(source)?)?;
    match fs::metadata(appearance_override) {
        Ok(_) => catalog.palette = parse_appearance_override(&read_source(appearance_override)?)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let games = prepare(games_output, catalog.games_tsv().as_bytes())?;
    let palette = prepare(palette_output, catalog.palette_tsv().as_bytes())?;
    fs::rename(&palette.0, palette_output)?;
    fs::rename(&games.0, games_output)?;
    Ok((catalog.games.len(), catalog.palette.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = include_str!("../../deploy/menu/games.sexp");
    const GAME: &str = r##"(:id "example" :title "Example" :system :nes :rom "/mnt/data/roms/nes/example.nes" :color "#ffaf87")"##;
    fn catalog_with_games(games: &str) -> String {
        let palette = PALETTE_ROLES
            .iter()
            .map(|key| format!(":{key} \"#123456\" "))
            .collect::<String>();
        format!("(:version 7 :palette ({palette}) :games ({games}))")
    }
    fn appearance(version: u8, extra: &str) -> String {
        let palette = PALETTE_ROLES
            .iter()
            .rev()
            .map(|key| format!(":{key} \"#abcdef\" "))
            .collect::<String>();
        format!("(:version {version} {extra} :palette ({palette}))")
    }
    #[test]
    fn checked_in_tsv_is_byte_identical() {
        let catalog = parse_catalog(SOURCE).unwrap();
        assert_eq!(
            catalog.games_tsv(),
            include_str!("../../deploy/menu/games.tsv")
        );
        assert_eq!(
            catalog.palette_tsv(),
            include_str!("../../deploy/menu/palette.tsv")
        );
    }
    #[test]
    fn common_lisp_strings_and_comments() {
        assert_eq!(
            read_form(
                r#"; line
            #| outer #| nested |# block |# (:TiTlE "a\nb\"c\\d") ; tail"#
            )
            .unwrap(),
            Form::List(vec![
                Form::Keyword("title".into()),
                Form::String("anb\"c\\d".into())
            ])
        );
        let catalog = parse_catalog(&catalog_with_games(
            &GAME.replace("Example", r#"A\nB\"C\\D"#),
        ))
        .unwrap();
        assert_eq!(catalog.games[0].title, "AnB\"C\\D");
        assert_eq!(read_form("+7.").unwrap(), Form::Integer(7));
    }
    #[test]
    fn reader_rejects_code_and_malformed_data() {
        for source in [
            "",
            "() ()",
            "#.(write-line \"bad\")",
            "#1=(a . #1#)",
            "'(1)",
            "`(1)",
            "(1 . 2)",
            "(:a |b|)",
            "(:a #x10)",
            "(:a \"unterminated)",
            "#| unfinished",
            "(1",
            ")",
            "999999999999999999999999",
        ] {
            assert!(read_form(source).is_err(), "accepted {source:?}");
        }
    }
    #[test]
    fn reader_has_byte_depth_and_node_limits() {
        assert!(read_form(&" ".repeat(MAXIMUM_CATALOG_BYTES + 1)).is_err());
        assert!(read_form(&format!("{}0{}", "(".repeat(40), ")".repeat(40))).is_err());
        assert!(read_form(&format!("({})", "0 ".repeat(MAXIMUM_NODES))).is_err());
        assert!(read_form(&format!("{}{}", "#|".repeat(40), "|#".repeat(40))).is_err());
        let padded = format!("7{}", " ".repeat(MAXIMUM_CATALOG_BYTES - 1));
        assert_eq!(read_form(&padded).unwrap(), Form::Integer(7));
    }
    #[test]
    fn catalog_requires_exact_schema_and_unique_fields() {
        let valid = catalog_with_games(GAME);
        for source in [
            valid.replace(":version 7", ":version 6"),
            valid.replace(":version 7", ":version 7 :version 7"),
            valid.replace(":version 7", ":version 7 :unknown 0"),
            valid.replace(":version 7", ""),
            valid.replace(":color \"#ffaf87\"", ""),
            valid.replace(":id \"example\"", ":id \"example\" :id \"other\""),
            valid.replace(":background \"#123456\"", ""),
            valid.replace(
                ":background \"#123456\"",
                ":background \"#123456\" :background \"#123456\"",
            ),
            catalog_with_games(""),
            catalog_with_games(&format!("{GAME} {GAME}")),
            catalog_with_games(&format!(
                "{GAME} {}",
                GAME.replace("example.nes", "other.nes")
            )),
            catalog_with_games(&format!(
                "{GAME} {}",
                GAME.replace(":id \"example\"", ":id \"other\"")
            )),
        ] {
            assert!(parse_catalog(&source).is_err(), "accepted {source}");
        }
    }
    #[test]
    fn game_count_and_order_are_preserved() {
        let games = (0..64)
            .map(|n| GAME.replace("example", &format!("game-{n}")))
            .collect::<Vec<_>>();
        let parsed = parse_catalog(&catalog_with_games(&games.join(" "))).unwrap();
        assert_eq!(parsed.games.len(), 64);
        assert_eq!(parsed.games[0].id, "game-0");
        assert_eq!(parsed.games[63].id, "game-63");
        assert!(
            parse_catalog(&catalog_with_games(&format!("{} {GAME}", games.join(" ")))).is_err()
        );
    }
    #[test]
    fn validates_all_system_routes_and_deck_apps() {
        for (system, suffix) in [
            ("nes", "nes"),
            ("gb", "gb"),
            ("gbc", "gbc"),
            ("gba", "gba"),
            ("zx", "tap"),
        ] {
            let game = GAME
                .replace(":nes", &format!(":{system}"))
                .replace("/nes/example.nes", &format!("/{system}/example.{suffix}"));
            assert!(
                parse_catalog(&catalog_with_games(&game)).is_ok(),
                "{system}"
            );
        }
        let deck = GAME.replace(":nes", ":deck").replace(
            "/mnt/data/roms/nes/example.nes",
            "/mnt/data/nes-deck/games/doom/example.wad",
        );
        assert!(parse_catalog(&catalog_with_games(&deck)).is_ok());
        for path in [
            "/tmp/example.nes",
            "/mnt/data/roms/gb/example.nes",
            "/mnt/data/roms/nes/example.gb",
            "/mnt/data/roms/nes//example.nes",
            "/mnt/data/roms/nes/./example.nes",
            "/mnt/data/roms/nes/../example.nes",
            "/mnt/data/roms/nes/example name.nes",
            "/mnt/data/roms/nes/example;name.nes",
        ] {
            assert!(
                parse_catalog(&catalog_with_games(
                    &GAME.replace("/mnt/data/roms/nes/example.nes", path)
                ))
                .is_err(),
                "{path}"
            );
        }
    }
    #[test]
    fn rejects_invalid_text_ids_and_colors() {
        for id in [
            "-a",
            "a-",
            "a--b",
            "UPPER",
            "a_b",
            "",
            "abcdefghijklmnopqrstuvwxyz123456789",
        ] {
            assert!(
                parse_catalog(&catalog_with_games(
                    &GAME.replace("\"example\"", &format!("\"{id}\""))
                ))
                .is_err(),
                "{id}"
            );
        }
        for title in [
            " leading",
            "trailing ",
            "",
            "tab\tvalue",
            "line\nvalue",
            "unicode-é",
            "012345678901234567890123456789012345678901234567890",
        ] {
            assert!(
                parse_catalog(&catalog_with_games(
                    &GAME.replace("\"Example\"", &format!("\"{title}\""))
                ))
                .is_err()
            );
        }
        for color in ["#123456", "#GG0000", "#00000", "0000000"] {
            assert!(
                parse_catalog(&catalog_with_games(&GAME.replace("#ffaf87", color))).is_err(),
                "{color}"
            );
        }
        for color in ["#800000", "#5f87af", "#080808", "#eeeeee"] {
            assert!(
                parse_catalog(&catalog_with_games(&GAME.replace("#ffaf87", color))).is_ok(),
                "{color}"
            );
        }
    }
    #[test]
    fn appearance_versions_icons_and_canonical_order() {
        for source in [
            appearance(2, ""),
            appearance(3, ":settings-icon \"old-cog-1\""),
        ] {
            let palette = parse_appearance_override(&source).unwrap();
            assert_eq!(
                palette
                    .iter()
                    .map(|(key, _)| key.as_str())
                    .collect::<Vec<_>>(),
                PALETTE_ROLES
            );
            assert!(palette.iter().all(|(_, color)| color == "#ABCDEF"));
        }
        for source in [
            appearance(1, ""),
            appearance(2, ":settings-icon \"old-cog\""),
            appearance(3, ""),
            appearance(3, ":settings-icon \"Bad Cog\""),
            appearance(3, ":settings-icon \"\""),
            appearance(3, ":settings-icon \"ok\" :extra 0"),
            appearance(3, ":settings-icon \"ok\" :settings-icon \"ok\""),
        ] {
            assert!(parse_appearance_override(&source).is_err());
        }
    }
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "retrodeck-catalog-{}-{}",
                std::process::id(),
                NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
        fn compile(&self) -> Result<(usize, usize)> {
            compile_catalog(
                &self.path("source"),
                &self.path("games"),
                &self.path("palette"),
                &self.path("override"),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn file_compilation_and_override_preserve_games() {
        let fixture = Fixture::new();
        fs::write(fixture.path("source"), SOURCE).unwrap();
        assert_eq!(fixture.compile().unwrap(), (1, 22));
        let games = fs::read(fixture.path("games")).unwrap();
        fs::write(
            fixture.path("override"),
            appearance(3, ":settings-icon \"cog\""),
        )
        .unwrap();
        assert_eq!(fixture.compile().unwrap(), (1, 22));
        assert_eq!(fs::read(fixture.path("games")).unwrap(), games);
        assert!(
            fs::read_to_string(fixture.path("palette"))
                .unwrap()
                .starts_with("background\t#ABCDEF\n")
        );
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 4);
    }
    #[test]
    fn validation_failure_keeps_outputs_and_leaves_no_temporaries() {
        let fixture = Fixture::new();
        fs::write(fixture.path("source"), SOURCE).unwrap();
        fixture.compile().unwrap();
        let games = fs::read(fixture.path("games")).unwrap();
        let palette = fs::read(fixture.path("palette")).unwrap();
        for source in [
            "#.(error \"no\")".to_owned(),
            " ".repeat(MAXIMUM_CATALOG_BYTES + 1),
        ] {
            fs::write(fixture.path("source"), source).unwrap();
            assert!(fixture.compile().is_err());
            assert_eq!(fs::read(fixture.path("games")).unwrap(), games);
            assert_eq!(fs::read(fixture.path("palette")).unwrap(), palette);
            assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 3);
        }
        fs::write(fixture.path("source"), SOURCE).unwrap();
        fs::write(fixture.path("override"), "(:version 3)").unwrap();
        assert!(fixture.compile().is_err());
        assert_eq!(fs::read(fixture.path("games")).unwrap(), games);
        assert_eq!(fs::read(fixture.path("palette")).unwrap(), palette);
    }
    #[test]
    fn output_prepare_failure_cleans_up_without_committing() {
        let fixture = Fixture::new();
        fs::write(fixture.path("source"), SOURCE).unwrap();
        fs::write(fixture.path("games"), "old games").unwrap();
        assert!(
            compile_catalog(
                &fixture.path("source"),
                &fixture.path("games"),
                &fixture.path("missing/palette"),
                &fixture.path("override")
            )
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(fixture.path("games")).unwrap(),
            "old games"
        );
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 2);
    }
}
