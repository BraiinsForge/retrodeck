//! Passworded, local-only ROM intake. HTTP parsing and multipart decoding use
//! axum; ZIP decoding uses zip. Uploaded bytes are never executed.
use crate::{
    catalog::{PALETTE_ROLES, parse_appearance_override},
    regular_file,
};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{ConnectInfo, DefaultBodyLimit, FromRequest, Multipart, Request, State},
    http::{HeaderMap, Method},
    response::Response,
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD},
};
use pbkdf2::pbkdf2_hmac_array;
use sha2::Sha256;
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    net::{Ipv4Addr, SocketAddr},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tokio::sync::Semaphore;

type Result<T> = std::result::Result<T, String>;
const UPLOAD_LIMIT: usize = 10 * 1024 * 1024;
const MULTIPART_LIMIT: usize = 12 * 1024 * 1024;
const SESSION_SECONDS: u64 = 28_800;
const LABELS: &[&str] = &[
    "Background",
    "Dark text",
    "Field",
    "Surface",
    "Inactive border",
    "Control border",
    "Footer",
    "Inactive text",
    "Text",
    "Bright white",
    "Title",
    "Volume off",
    "Volume on",
    "Selected item",
    "Wi-Fi active",
    "Wi-Fi focus",
    "Wi-Fi active border",
    "Field label",
    "Accent",
    "Active control",
    "Control surface",
    "Muted control",
];
const LOGO: [u8; 48] = [
    206, 237, 102, 102, 204, 13, 0, 11, 3, 115, 0, 131, 0, 12, 0, 13, 0, 8, 17, 31, 136, 137, 0,
    14, 220, 204, 110, 230, 221, 221, 217, 153, 187, 187, 103, 99, 110, 14, 236, 204, 221, 220,
    153, 159, 187, 185, 51, 62,
];

#[derive(Clone)]
pub struct Config {
    pub data_root: PathBuf,
    pub asset_directory: PathBuf,
    pub address: Ipv4Addr,
    pub port: u16,
    /// None disables dashboard restart for an embedding application.
    pub restart_command: Option<Vec<String>>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            data_root: "/mnt/data".into(),
            asset_directory: "/mnt/data/nes-deck/uploader".into(),
            address: Ipv4Addr::UNSPECIFIED,
            port: 8080,
            restart_command: Some(vec!["/etc/init.d/nes-deck".into(), "restart".into()]),
        }
    }
}
impl Config {
    fn path(&self, relative: &str) -> PathBuf {
        self.data_root.join(relative)
    }
    fn password(&self) -> PathBuf {
        self.path("nes-deck/uploader/password.conf")
    }
}

/// Python provisioning writes version=1 and unpadded standard base64 fields.
/// Inspect the opened descriptor, not a separate pathname metadata snapshot.
#[derive(Clone)]
pub struct PasswordConfig {
    iterations: u32,
    salt: Vec<u8>,
    digest: Vec<u8>,
}
impl PasswordConfig {
    pub fn read(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(path)
            .map_err(|e| format!("cannot open uploader password configuration: {e}"))?;
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file()
            || metadata.mode() & 0o077 != 0
            || unsafe { libc::geteuid() } == 0 && metadata.uid() != 0
        {
            return Err("uploader password configuration must be private and root-owned".into());
        }
        let mut bytes = Vec::new();
        file.take(1025)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.is_empty() || bytes.len() > 1024 {
            return Err("uploader password configuration has invalid size".into());
        }
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| "uploader password configuration is not UTF-8")?;
        let lines = text.split('\n').collect::<Vec<_>>();
        if lines.len() != 5 || lines[0] != "version=1" || !lines[4].is_empty() {
            return Err("uploader password configuration has an invalid schema".into());
        }
        let mut fields = HashMap::new();
        for line in &lines[1..4] {
            let (name, value) = line
                .split_once('=')
                .ok_or("uploader password configuration has a malformed field")?;
            if !["iterations", "salt", "digest"].contains(&name)
                || fields.insert(name, value).is_some()
            {
                return Err(
                    "uploader password configuration has unknown or repeated fields".into(),
                );
            }
        }
        let iterations = fields["iterations"]
            .parse::<u32>()
            .ok()
            .filter(|n| (100_000..=1_000_000).contains(n))
            .ok_or("uploader password configuration has invalid iterations")?;
        let salt = STANDARD_NO_PAD
            .decode(fields["salt"])
            .map_err(|_| "uploader password configuration has an invalid salt")?;
        let digest = STANDARD_NO_PAD
            .decode(fields["digest"])
            .map_err(|_| "uploader password configuration has an invalid digest")?;
        if salt.len() != 16 || digest.len() != 32 {
            return Err("uploader password configuration has invalid byte lengths".into());
        }
        Ok(Self {
            iterations,
            salt,
            digest,
        })
    }
    pub fn verify(&self, password: &[u8]) -> bool {
        password.len() <= 128
            && bool::from(
                pbkdf2_hmac_array::<Sha256, 32>(password, &self.salt, self.iterations)
                    .as_slice()
                    .ct_eq(&self.digest),
            )
    }
}
pub fn verify_password_stdin(path: &Path) -> Result<u8> {
    let config = PasswordConfig::read(path)?;
    let mut password = Vec::new();
    std::io::stdin()
        .take(129)
        .read_to_end(&mut password)
        .map_err(|e| e.to_string())?;
    Ok(u8::from(!config.verify(&password)))
}
fn token() -> Result<String> {
    let mut bytes = [0; 32];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|e| format!("random token failed: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
fn same_token(a: &str, b: &str) -> bool {
    a.len() == b.len() && bool::from(a.as_bytes().ct_eq(b.as_bytes()))
}
#[derive(Clone)]
struct Session {
    id: String,
    csrf: String,
    expires: Instant,
}
struct Attempt {
    failures: u8,
    blocked_until: Instant,
}
#[derive(Default)]
struct Auth {
    sessions: HashMap<String, Session>,
    attempts: HashMap<Ipv4Addr, Attempt>,
}
struct App {
    config: Config,
    auth: Mutex<Auth>,
    storage: Mutex<()>,
    login: Arc<Semaphore>,
}
impl App {
    fn session(&self, headers: &HeaderMap) -> Option<Session> {
        let cookie = header(headers, "cookie")?
            .split(';')
            .filter_map(|s| s.trim().split_once('='))
            .find(|(k, _)| *k == "deck_rom_session")?
            .1;
        if cookie.len() != 43 {
            return None;
        }
        let mut auth = self.auth.lock().unwrap();
        auth.sessions.retain(|_, s| s.expires > Instant::now());
        auth.sessions.get(cookie).cloned()
    }
    fn create_session(&self) -> Result<Session> {
        let session = Session {
            id: token()?,
            csrf: token()?,
            expires: Instant::now() + Duration::from_secs(SESSION_SECONDS),
        };
        let mut auth = self.auth.lock().unwrap();
        auth.sessions.retain(|_, s| s.expires > Instant::now());
        if auth.sessions.len() >= 1024 {
            return Err("too many active sessions".into());
        }
        auth.sessions.insert(session.id.clone(), session.clone());
        Ok(session)
    }
    fn blocked(&self, host: Ipv4Addr) -> Option<u64> {
        self.auth
            .lock()
            .unwrap()
            .attempts
            .get(&host)
            .and_then(|a| a.blocked_until.checked_duration_since(Instant::now()))
            .map(|d| d.as_secs() + u64::from(d.subsec_nanos() > 0))
    }
    fn record_login(&self, host: Ipv4Addr, success: bool) {
        let mut auth = self.auth.lock().unwrap();
        if success {
            auth.attempts.remove(&host);
        } else {
            let attempt = auth.attempts.entry(host).or_insert(Attempt {
                failures: 0,
                blocked_until: Instant::now(),
            });
            attempt.failures += 1;
            if attempt.failures >= 5 {
                attempt.failures = 0;
                attempt.blocked_until = Instant::now() + Duration::from_secs(300);
            }
        }
    }
    fn restart(&self) -> Result<()> {
        if let Some(command) = &self.config.restart_command {
            let (program, args) = command.split_first().ok_or("restart command is empty")?;
            let mut child = std::process::Command::new(program)
                .args(args)
                .spawn()
                .map_err(|e| format!("restart failed: {e}"))?;
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                match child.try_wait() {
                    Ok(Some(status)) if status.success() => break,
                    Ok(Some(status)) => return Err(format!("restart failed with status {status}")),
                    Ok(None) if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(20))
                    }
                    result => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(match result {
                            Err(e) => format!("restart failed: {e}"),
                            _ => "restart timed out".into(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}
fn host_allowed(headers: &HeaderMap, port: u16) -> bool {
    header(headers, "host")
        .and_then(|h| h.rsplit_once(':'))
        .is_some_and(|(ip, p)| {
            p == port.to_string() && ip.parse::<Ipv4Addr>().is_ok_and(|v| v.to_string() == ip)
        })
}
fn same_origin(headers: &HeaderMap) -> bool {
    header(headers, "origin").is_none_or(|origin| {
        origin == "null"
            || header(headers, "host").is_some_and(|host| origin == format!("http://{host}"))
    })
}
fn content_length(headers: &HeaderMap, limit: usize) -> bool {
    header(headers, "content-length")
        .and_then(|s| s.parse::<usize>().ok())
        .is_some_and(|n| n <= limit)
}
fn media_type(headers: &HeaderMap, expected: &str) -> bool {
    header(headers, "content-type").is_some_and(|s| {
        s.split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case(expected)
    })
}
fn response(status: u16, content_type: &str, body: impl Into<Body>) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", content_type)
        .body(body.into())
        .unwrap()
}
fn plain(status: u16, text: &str) -> Response {
    response(status, "text/plain; charset=utf-8", format!("{text}\n"))
}
fn html(status: u16, body: String) -> Response {
    response(status, "text/html; charset=utf-8", body)
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn page(
    session: Option<&Session>,
    error: Option<&str>,
    notice: Option<&str>,
    palette: Option<&HashMap<String, String>>,
) -> String {
    let mut out = include_str!("head.html").to_owned();
    if let Some(s) = session {
        out.push_str(&format!("<form action=\"/logout\" method=\"post\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><button type=\"submit\" class=\"quiet-button\">Sign out</button></form>",s.csrf));
    }
    out.push_str("</header>");
    if let Some(s) = session {
        if let Some(e) = error {
            out.push_str(&format!(
                "<p class=\"message error\" role=\"alert\"><strong>Not filed:</strong> {}</p>",
                escape(e)
            ));
        }
        if let Some(n) = notice {
            out.push_str(&format!(
                "<p class=\"message\" role=\"status\"><strong>Filed:</strong> {}</p>",
                escape(n)
            ));
        }
        out.push_str(&include_str!("upload.html").replace("~A", &s.csrf));
        if let Some(p) = palette {
            out.push_str(&format!("<section class=\"palette-section\"><h2>Dashboard appearance</h2><form action=\"/palette\" method=\"post\"><input type=\"hidden\" name=\"csrf\" value=\"{}\"><h3>Colors</h3><p class=\"hint\">#RRGGBB</p><div class=\"palette-grid\">",s.csrf));
            for (name, label) in PALETTE_ROLES.iter().zip(LABELS) {
                let value = &p[*name];
                out.push_str(&format!("<label><span>{label}</span><span class=\"palette-inputs\"><input class=\"palette-picker\" type=\"color\" value=\"{value}\" data-palette-picker=\"palette-{name}\" aria-label=\"Choose {label}\"><input class=\"palette-hex\" id=\"palette-{name}\" type=\"text\" name=\"{name}\" value=\"{value}\" pattern=\"#[0-9A-Fa-f]{{6}}\" minlength=\"7\" maxlength=\"7\" spellcheck=\"false\" autocomplete=\"off\" required></span></label>"));
            }
            out.push_str("</div><button type=\"submit\" class=\"primary-button\">Apply appearance</button></form></section>");
        }
    } else {
        if let Some(e) = error {
            out.push_str(&format!(
                "<p class=\"message error\" role=\"alert\"><strong>Error:</strong> {}</p>",
                escape(e)
            ));
        }
        out.push_str(include_str!("login.html"));
    }
    out.push_str("</main></body></html>");
    out
}
fn read_text(path: &Path, limit: u64, missing_ok: bool) -> Result<Option<String>> {
    let bytes = regular_file::read_regular(path, 0, limit, "uploader data")?;
    if !missing_ok && bytes.is_none() {
        return Err(format!("file is missing: {}", path.display()));
    }
    bytes
        .map(|b| String::from_utf8(b).map_err(|_| "file is not UTF-8".into()))
        .transpose()
}
fn normalize_rgb(value: &str) -> Option<String> {
    (value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit))
    .then(|| value.to_ascii_uppercase())
}
fn validate_palette(p: &HashMap<String, String>) -> Result<()> {
    if p.len() != PALETTE_ROLES.len()
        || PALETTE_ROLES.iter().any(|name| {
            p.get(*name)
                .is_none_or(|v| normalize_rgb(v).as_ref() != Some(v))
        })
    {
        return Err(
            "palette must contain every dashboard color exactly once in canonical #RRGGBB".into(),
        );
    }
    Ok(())
}
fn parse_palette_tsv(text: &str) -> Result<HashMap<String, String>> {
    let mut p = HashMap::new();
    let mut icon = false;
    for line in text.lines() {
        let (key, value) = line
            .split_once('\t')
            .ok_or("appearance contains a malformed entry")?;
        if key == "settings-icon" {
            if icon
                || value.is_empty()
                || value.len() > 64
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err("invalid settings icon".into());
            }
            icon = true;
            continue;
        }
        if !PALETTE_ROLES.contains(&key)
            || p.insert(
                key.into(),
                normalize_rgb(value).ok_or("invalid palette color")?,
            )
            .is_some()
        {
            return Err("unknown or repeated palette entry".into());
        }
    }
    validate_palette(&p)?;
    Ok(p)
}
fn current_palette(config: &Config) -> Result<HashMap<String, String>> {
    let mut base = None;
    for path in ["nes-deck/menu/palette.tsv", "nes-deck/state/palette.tsv"] {
        if let Ok(Some(text)) = read_text(&config.path(path), 4096, false) {
            if let Ok(p) = parse_palette_tsv(&text) {
                base = Some(p);
                break;
            }
        }
    }
    let mut base = base.ok_or("read installed dashboard appearance")?;
    if let Ok(Some(text)) = read_text(
        &config.path("nes-deck/state/dashboard-palette.sexp"),
        4096,
        true,
    ) {
        if let Ok(p) = parse_appearance_override(&text) {
            base = p.into_iter().collect();
        }
    }
    validate_palette(&base)?;
    Ok(base)
}
fn dashboard(app: &App, s: &Session, error: Option<&str>, notice: Option<&str>) -> String {
    let catalog = read_catalog(&app.config.path("nes-deck/uploads/games.tsv"), true);
    let palette = current_palette(&app.config);
    let error = error
        .or_else(|| {
            catalog
                .is_err()
                .then_some("The upload catalog cannot be read.")
        })
        .or_else(|| {
            palette
                .is_err()
                .then_some("The dashboard appearance cannot be read.")
        });
    page(Some(s), error, notice, palette.as_ref().ok())
}
fn encode_palette(p: &HashMap<String, String>) -> String {
    let entries = PALETTE_ROLES
        .iter()
        .map(|n| format!(":{n} \"{}\"", p[*n]))
        .collect::<Vec<_>>()
        .join("\n   ");
    format!("(:version 2\n :palette\n  ({entries}))\n")
}
/// Trusted state directories must not contain symlink ancestors. All transient
/// files use exclusive creation and mode 0600 before writing any owner bytes.
fn ensure_directory(directory: &Path) -> Result<()> {
    // Inspect existing ancestors before create_dir_all can follow them.
    for ancestor in directory.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if meta.is_dir() => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err("storage directory is not a real directory".into()),
        }
    }
    fs::create_dir_all(directory).map_err(|e| e.to_string())
}
fn install(path: &Path, bytes: &[u8], overwrite: bool) -> Result<()> {
    let directory = path.parent().ok_or("missing storage directory")?;
    ensure_directory(directory)?;
    let temporary = directory.join(format!(".uploader-{}", token()?));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        if overwrite {
            fs::rename(&temporary, path).map_err(|e| e.to_string())?;
        } else {
            fs::hard_link(&temporary, path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    "a ROM with this name already exists".into()
                } else {
                    e.to_string()
                }
            })?;
        }
        File::open(directory)
            .and_then(|d| d.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result
}
#[derive(Clone)]
struct System {
    extension: &'static str,
    color: &'static str,
    maximum: usize,
}
fn system(name: &str) -> Result<System> {
    let (extension, color, maximum) = match name {
        "nes" => ("nes", "#FF5F00", 8_388_608),
        "gb" => ("gb", "#87AF87", 8_388_608),
        "gbc" => ("gbc", "#5F87D7", 8_388_608),
        "zx" => ("tap", "#AF87D7", 8_388_608),
        "gba" => ("gba", "#875FD7", 33_554_432),
        _ => return Err("choose a supported system".into()),
    };
    Ok(System {
        extension,
        color,
        maximum,
    })
}
fn validate_rom(name: &str, bytes: &[u8]) -> Result<()> {
    let spec = system(name)?;
    if bytes.is_empty() || bytes.len() > spec.maximum {
        return Err(format!(
            "the ROM must contain 1 through {} bytes",
            spec.maximum
        ));
    }
    match name {
        "nes" => {
            if bytes.len() < 16 || &bytes[..4] != b"NES\x1a" {
                return Err("the file has no iNES header".into());
            }
        }
        "gb" | "gbc" => {
            if bytes.len() < 336 || bytes[260..308] != LOGO {
                return Err("the file has no valid Game Boy header".into());
            }
            let checksum = bytes[308..333]
                .iter()
                .fold(0u8, |n, b| n.wrapping_sub(*b).wrapping_sub(1));
            if checksum != bytes[333] {
                return Err("the Game Boy header checksum is invalid".into());
            }
            if name == "gbc" && ![128, 192].contains(&bytes[323]) {
                return Err("the ROM does not advertise Game Boy Color support".into());
            }
        }
        "zx" => {
            if bytes.len() < 4 {
                return Err("the TAP file is too short".into());
            }
            let mut offset = 0;
            while offset < bytes.len() {
                if bytes.len() - offset < 2 {
                    return Err("the TAP file ends inside a block header".into());
                }
                let len = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as usize;
                offset += 2;
                if len < 2 || len > bytes.len() - offset {
                    return Err("the TAP file has an invalid block length".into());
                }
                if bytes[offset..offset + len].iter().fold(0, |n, b| n ^ b) != 0 {
                    return Err("the TAP file has an invalid block checksum".into());
                }
                offset += len;
            }
        }
        _ => {}
    }
    Ok(())
}
fn extension(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default()
}
/// The legacy intake accepts a complete single-disk ZIP, not a ZIP embedded
/// in another file. Apply those envelope constraints before the ZIP decoder.
fn validate_zip_envelope(bytes: &[u8]) -> Result<()> {
    let end = bytes
        .windows(4)
        .rposition(|w| w == b"PK\x05\x06")
        .ok_or("the ZIP file is malformed")?;
    let header = bytes
        .get(end..end + 22)
        .ok_or("the ZIP file is malformed")?;
    let short = |offset| u16::from_le_bytes([header[offset], header[offset + 1]]);
    if end + 22 + short(20) as usize != bytes.len()
        || short(4) != 0
        || short(6) != 0
        || short(8) != short(10)
    {
        return Err("the ZIP file is malformed".into());
    }
    let offset = u32::from_le_bytes(header[16..20].try_into().unwrap()) as usize;
    if short(10) != 0
        && offset.checked_add(4).and_then(|end| bytes.get(offset..end)) != Some(b"PK\x01\x02")
    {
        return Err("the ZIP file is malformed".into());
    }
    Ok(())
}
fn decode_upload(name: &str, filename: &str, bytes: &[u8]) -> Result<Vec<u8>> {
    let spec = system(name)?;
    if bytes.len() > UPLOAD_LIMIT {
        return Err("the upload exceeds 10 MiB".into());
    }
    if extension(filename) == spec.extension {
        validate_rom(name, bytes)?;
        return Ok(bytes.to_vec());
    }
    if extension(filename) != "zip" {
        return Err(format!(
            "choose a .{} file or a ZIP containing one",
            spec.extension
        ));
    }
    validate_zip_envelope(bytes)?;
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "the ZIP file is malformed")?;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let file = archive
            .by_index_raw(index)
            .map_err(|_| "the ZIP file is malformed")?;
        if !file.is_dir() {
            files.push(index);
        }
    }
    if files.len() != 1 {
        return Err("the ZIP must contain exactly one ROM".into());
    }
    let mut file = archive
        .by_index(files[0])
        .map_err(|_| "the ZIP contains an unsafe or encrypted member")?;
    if file.name().contains(['/', '\\'])
        || file.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
        || file.encrypted()
    {
        return Err("the ZIP contains an unsafe or encrypted member".into());
    }
    if extension(file.name()) != spec.extension
        || file.size() == 0
        || file.size() > spec.maximum as u64
    {
        return Err(format!(
            "the ZIP must contain one .{} file of a valid size",
            spec.extension
        ));
    }
    let expected = file.size() as usize;
    let mut decoded = Vec::with_capacity(expected.min(spec.maximum));
    (&mut file)
        .take(expected as u64 + 1)
        .read_to_end(&mut decoded)
        .map_err(|_| "the ZIP member could not be read safely")?;
    if decoded.len() != expected {
        return Err("the ZIP member could not be read safely".into());
    }
    validate_rom(name, &decoded)?;
    Ok(decoded)
}
fn title_valid(title: &str) -> bool {
    let chars = title.chars().collect::<Vec<_>>();
    (1..=64).contains(&chars.len())
        && !chars[0].is_whitespace()
        && !chars[chars.len() - 1].is_whitespace()
        && chars
            .iter()
            .all(|c| !(*c < '\u{20}' || ('\u{7f}'..='\u{9f}').contains(c)))
}
fn slugify(title: &str) -> String {
    let mut slug = String::new();
    for c in title.to_lowercase().chars() {
        if slug.len() >= 32 {
            break;
        }
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_end_matches('-').into()
}
type Entry = [String; 5];
fn read_catalog(path: &Path, missing_ok: bool) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    if let Some(text) = read_text(path, 65536, missing_ok)? {
        for line in text.lines() {
            if line.len() > 4095 {
                return Err("bufio.Scanner: token too long".into());
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields = line.split('\t').map(str::to_owned).collect::<Vec<_>>();
            let entry = fields
                .try_into()
                .map_err(|_| "catalog contains a malformed line")?;
            entries.push(entry);
            if entries.len() > 64 {
                return Err("catalog contains too many games".into());
            }
        }
    }
    Ok(entries)
}
fn add_rom(
    app: &App,
    name: &str,
    title: &str,
    filename: &str,
    upload: &[u8],
) -> Result<Option<String>> {
    let _storage = app.storage.lock().unwrap();
    if !title_valid(title) {
        return Err("enter a title of 1 through 64 printable characters".into());
    }
    let spec = system(name)?;
    let slug = slugify(title);
    if slug.is_empty() {
        return Err("the title needs at least one ASCII letter or number for its filename".into());
    }
    let bytes = decode_upload(name, filename, upload)?;
    let base = read_catalog(&app.config.path("nes-deck/menu/games.tsv"), false)
        .map_err(|e| format!("read built-in catalog: {e}"))?;
    let catalog_path = app.config.path("nes-deck/uploads/games.tsv");
    let mut uploads =
        read_catalog(&catalog_path, true).map_err(|e| format!("read upload catalog: {e}"))?;
    let identifier = format!("upload-{name}-{slug}")
        .chars()
        .take(48)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned();
    let destination = app
        .config
        .path(&format!("roms/{name}/{slug}.{}", spec.extension));
    let destination_text = destination
        .to_str()
        .ok_or("storage path is not UTF-8")?
        .to_owned();
    if base.len() + uploads.len() >= 64 {
        return Err("the menu catalog is full".into());
    }
    if base
        .iter()
        .chain(&uploads)
        .any(|e| e[0] == identifier || e[3] == destination_text)
    {
        return Err("a game with this title is already cataloged".into());
    }
    install(&destination, &bytes, false)?;
    uploads.push([
        identifier,
        title.into(),
        name.into(),
        destination_text,
        spec.color.into(),
    ]);
    uploads.sort_by(|a, b| a[2].cmp(&b[2]).then(a[1].cmp(&b[1])));
    let text = uploads
        .iter()
        .map(|e| format!("{}\n", e.join("\t")))
        .collect::<String>();
    if let Err(e) = install(&catalog_path, text.as_bytes(), true) {
        let _ = fs::remove_file(destination);
        return Err(format!("save upload catalog: {e}"));
    }
    Ok(app.restart().err())
}
fn redirect() -> Response {
    let mut r = plain(303, "");
    r.headers_mut().insert("location", "/".parse().unwrap());
    r
}
fn cookie(response: &mut Response, value: &str, age: i64) {
    response.headers_mut().insert(
        "set-cookie",
        format!("deck_rom_session={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age={age}")
            .parse()
            .unwrap(),
    );
}
fn reject_blocked(app: &App, host: Ipv4Addr) -> Option<Response> {
    app.blocked(host).map(|seconds| {
        let mut r = html(
            429,
            page(
                None,
                Some("Too many attempts. Wait five minutes before trying again."),
                None,
                None,
            ),
        );
        r.headers_mut()
            .insert("retry-after", seconds.to_string().parse().unwrap());
        r
    })
}
async fn url_form(request: Request, limit: usize) -> Result<Vec<(String, String)>> {
    let bytes = to_bytes(request.into_body(), limit)
        .await
        .map_err(|_| "form too large or malformed")?;
    // Match the old UTF-8 decoder's rejection rather than replacing invalid bytes.
    if std::str::from_utf8(&bytes).is_err() {
        return Err("form is not UTF-8".into());
    }
    for component in bytes.split(|b| *b == b'&' || *b == b'=') {
        percent_encoding::percent_decode(component)
            .decode_utf8()
            .map_err(|_| "form is not UTF-8")?;
    }
    serde_urlencoded::from_bytes(&bytes).map_err(|_| "form is malformed".into())
}
fn value<'a>(form: &'a [(String, String)], name: &str) -> &'a str {
    form.iter()
        .find(|(key, _)| key == name)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}
async fn dispatch(app: Arc<App>, peer: SocketAddr, request: Request) -> Response {
    let headers = request.headers().clone();
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    if !host_allowed(&headers, app.config.port) {
        return plain(421, "Use one of this Deck's IPv4 addresses on port 8080.");
    }
    let session = app.session(&headers);
    if path == "/" {
        if method != Method::GET {
            let mut r = plain(405, "Method not allowed");
            r.headers_mut().insert("allow", "GET".parse().unwrap());
            return r;
        }
        return html(
            200,
            match &session {
                Some(s) => dashboard(&app, s, None, None),
                None => page(None, None, None, None),
            },
        );
    }
    if let Some((file, kind)) = match path.as_str() {
        "/assets/paper.css" => Some(("uploader-paper.css", "text/css; charset=utf-8")),
        "/assets/palette.js" => Some(("uploader-palette.js", "text/javascript; charset=utf-8")),
        _ => None,
    } {
        if method != Method::GET {
            return plain(405, "Method not allowed");
        }
        return match read_text(&app.config.asset_directory.join(file), 65536, false) {
            Ok(Some(body)) => response(200, kind, body),
            _ => plain(500, "Asset unavailable"),
        };
    }
    if !["/login", "/logout", "/upload", "/palette", "/palette/reset"].contains(&path.as_str()) {
        return plain(404, "404 page not found");
    }
    if path == "/login" && method != Method::POST {
        let mut response = plain(405, "Method not allowed");
        response
            .headers_mut()
            .insert("allow", "POST".parse().unwrap());
        return response;
    }
    if path == "/login" && !same_origin(&headers) {
        return plain(403, "Cross-origin request rejected");
    }
    if method != Method::POST || !same_origin(&headers) {
        return plain(403, "Request rejected");
    }
    if path == "/login" {
        if !media_type(&headers, "application/x-www-form-urlencoded") {
            return plain(415, "Unsupported form encoding");
        }
        if !content_length(&headers, 512) {
            return html(
                400,
                page(None, Some("The login form was malformed."), None, None),
            );
        }
        let form = match url_form(request, 512).await {
            Ok(f) => f,
            Err(_) => {
                return html(
                    400,
                    page(None, Some("The login form was malformed."), None, None),
                );
            }
        };
        let host = match peer.ip() {
            std::net::IpAddr::V4(v) => v,
            _ => return plain(403, "Request rejected"),
        };
        if let Some(r) = reject_blocked(&app, host) {
            return r;
        }
        let permit =
            match tokio::time::timeout(Duration::from_secs(2), app.login.clone().acquire_owned())
                .await
            {
                Ok(Ok(g)) => g,
                _ => {
                    let mut r = html(
                        429,
                        page(
                            None,
                            Some("Another sign-in is being checked. Try again in a moment."),
                            None,
                            None,
                        ),
                    );
                    r.headers_mut().insert("retry-after", "3".parse().unwrap());
                    return r;
                }
            };
        if let Some(r) = reject_blocked(&app, host) {
            return r;
        }
        let password = value(&form, "password").as_bytes().to_vec();
        let config = app.config.password();
        let app_for_login = app.clone();
        // Keep serialization and rate-limit accounting inside the worker, even
        // when a disconnected client's request future is cancelled.
        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let success = PasswordConfig::read(&config).is_ok_and(|c| c.verify(&password));
            app_for_login.record_login(host, success);
            if success {
                app_for_login.create_session().map(Some)
            } else {
                Ok(None)
            }
        })
        .await;
        return match result {
            Ok(Ok(Some(s))) => {
                let mut r = redirect();
                cookie(&mut r, &s.id, SESSION_SECONDS as i64);
                r
            }
            Ok(Ok(None)) => html(
                401,
                page(None, Some("That password was not accepted."), None, None),
            ),
            _ => plain(503, "Session capacity reached"),
        };
    }
    let s = match session {
        Some(s) => s,
        None => {
            return plain(
                if path == "/logout" { 403 } else { 401 },
                if path == "/logout" {
                    "Request rejected"
                } else {
                    "Authentication required"
                },
            );
        }
    };
    if path == "/upload" {
        if !content_length(&headers, MULTIPART_LIMIT)
            || !media_type(&headers, "multipart/form-data")
        {
            return html(
                400,
                dashboard(
                    &app,
                    &s,
                    Some("The upload form was malformed or too large."),
                    None,
                ),
            );
        }
        let (parts, body) = request.into_parts();
        let bytes = match to_bytes(body, MULTIPART_LIMIT).await {
            Ok(b) => b,
            Err(_) => {
                return html(
                    400,
                    dashboard(
                        &app,
                        &s,
                        Some("The upload form was malformed or too large."),
                        None,
                    ),
                );
            }
        };
        let limited = Request::from_parts(parts, Body::from(bytes));
        let mut multipart = match Multipart::from_request(limited, &()).await {
            Ok(m) => m,
            Err(_) => {
                return html(
                    400,
                    dashboard(
                        &app,
                        &s,
                        Some("The upload form was malformed or too large."),
                        None,
                    ),
                );
            }
        };
        let mut form = Vec::new();
        let mut files = Vec::new();
        let mut count = 0;
        loop {
            match multipart.next_field().await {
                Ok(Some(field)) => {
                    count += 1;
                    if count > 16 {
                        return html(
                            400,
                            dashboard(
                                &app,
                                &s,
                                Some("The upload form was malformed or too large."),
                                None,
                            ),
                        );
                    }
                    let key = field.name().unwrap_or("").to_owned();
                    let filename = field.file_name().map(str::to_owned);
                    let data = match field.bytes().await {
                        Ok(b) => b,
                        Err(_) => {
                            return html(
                                400,
                                dashboard(
                                    &app,
                                    &s,
                                    Some("The upload form was malformed or too large."),
                                    None,
                                ),
                            );
                        }
                    };
                    if let Some(filename) = filename.filter(|_| key == "rom") {
                        files.push((filename, data));
                    } else {
                        if data.len() > 4096 {
                            return html(
                                400,
                                dashboard(
                                    &app,
                                    &s,
                                    Some("The upload form was malformed or too large."),
                                    None,
                                ),
                            );
                        }
                        let text = match std::str::from_utf8(&data) {
                            Ok(t) => t.to_owned(),
                            Err(_) => return plain(400, "Malformed form"),
                        };
                        form.push((key, text));
                    }
                }
                Ok(None) => break,
                Err(_) => {
                    return html(
                        400,
                        dashboard(
                            &app,
                            &s,
                            Some("The upload form was malformed or too large."),
                            None,
                        ),
                    );
                }
            }
        }
        if !same_token(value(&form, "csrf"), &s.csrf) {
            return plain(403, "Request rejected");
        }
        if files.len() != 1 {
            return html(
                400,
                dashboard(
                    &app,
                    &s,
                    Some("Choose exactly one ROM or single-ROM ZIP."),
                    None,
                ),
            );
        }
        let (filename, bytes) = files.remove(0);
        let name = value(&form, "system").to_owned();
        let title = value(&form, "title").to_owned();
        let a = app.clone();
        let t = title.clone();
        let result =
            tokio::task::spawn_blocking(move || add_rom(&a, &name, &t, &filename, &bytes)).await;
        return match result {
            Ok(Ok(restart)) => html(
                200,
                dashboard(
                    &app,
                    &s,
                    None,
                    Some(&format!(
                        "{title} {}",
                        if restart.is_some() {
                            "was saved. The dashboard will pick it up after its next restart."
                        } else {
                            "was validated, filed, and added to the dashboard."
                        }
                    )),
                ),
            ),
            Ok(Err(e)) => html(422, dashboard(&app, &s, Some(&e), None)),
            Err(_) => plain(500, "Upload failed"),
        };
    }
    if !media_type(&headers, "application/x-www-form-urlencoded") {
        return plain(415, "Unsupported form encoding");
    }
    if !content_length(&headers, 4096) {
        return plain(400, "Malformed form");
    }
    let form = match url_form(request, 4096).await {
        Ok(f) => f,
        Err(_) => return plain(400, "Malformed form"),
    };
    if form.iter().filter(|(k, _)| k == "csrf").count() != 1
        || !same_token(value(&form, "csrf"), &s.csrf)
    {
        return plain(403, "Request rejected");
    }
    if path == "/logout" {
        app.auth.lock().unwrap().sessions.remove(&s.id);
        let mut r = redirect();
        cookie(&mut r, "", -1);
        return r;
    }
    let reset = path == "/palette/reset";
    let mut palette = HashMap::new();
    for (key, v) in form {
        if key == "csrf" {
            continue;
        }
        if reset || !PALETTE_ROLES.contains(&key.as_str()) || palette.contains_key(&key) {
            return html(
                400,
                dashboard(
                    &app,
                    &s,
                    Some("The appearance form contains an unknown or repeated field."),
                    None,
                ),
            );
        }
        let color = match normalize_rgb(&v) {
            Some(c) => c,
            None => {
                return html(
                    400,
                    dashboard(
                        &app,
                        &s,
                        Some("Every color must be a full RGB value like #12ABEF."),
                        None,
                    ),
                );
            }
        };
        palette.insert(key, color);
    }
    if !reset {
        if let Err(e) = validate_palette(&palette) {
            return html(422, dashboard(&app, &s, Some(&e), None));
        }
    }
    let a = app.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _storage = a.storage.lock().unwrap();
        let path = a.config.path("nes-deck/state/dashboard-palette.sexp");
        if reset {
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        } else {
            install(&path, encode_palette(&palette).as_bytes(), true)?;
        }
        a.restart()
            .map_err(|e| format!("appearance was saved, but the dashboard could not reload: {e}"))
    })
    .await;
    match result {
        Ok(Ok(())) => html(
            200,
            dashboard(
                &app,
                &s,
                None,
                Some(if reset {
                    "Dashboard appearance was reset and applied."
                } else {
                    "Dashboard appearance was saved and applied."
                }),
            ),
        ),
        Ok(Err(e)) => html(422, dashboard(&app, &s, Some(&e), None)),
        Err(_) => plain(500, "Appearance update failed"),
    }
}
async fn handler(
    State(app): State<Arc<App>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    let mut response =
        match tokio::time::timeout(Duration::from_secs(35), dispatch(app, peer, request)).await {
            Ok(r) => r,
            Err(_) => plain(408, "Request timed out"),
        };
    for (key, value) in [
        ("cache-control", "no-store"),
        (
            "content-security-policy",
            "default-src 'none'; img-src 'self'; style-src 'self'; script-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
        ),
        ("cross-origin-opener-policy", "same-origin"),
        ("cross-origin-resource-policy", "same-origin"),
        ("referrer-policy", "no-referrer"),
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
    ] {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static(key),
            value.parse().unwrap(),
        );
    }
    response
}
/// Bind once, validate the private password file before accepting requests, and
/// stop on SIGTERM/SIGINT. The returned address supports ephemeral-port tests.
pub async fn serve(
    config: Config,
    ready: impl FnOnce(SocketAddr),
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let mut config = config;
    if !config.data_root.is_absolute() {
        config.data_root = std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(&config.data_root);
    }
    PasswordConfig::read(&config.password())?;
    let listener = tokio::net::TcpListener::bind((config.address, config.port))
        .await
        .map_err(|e| e.to_string())?;
    let address = listener.local_addr().map_err(|e| e.to_string())?;
    config.port = address.port();
    let app = Arc::new(App {
        config,
        auth: Mutex::new(Auth::default()),
        storage: Mutex::new(()),
        login: Arc::new(Semaphore::new(1)),
    });
    let router = Router::new()
        .fallback(handler)
        .layer(DefaultBodyLimit::max(MULTIPART_LIMIT))
        .with_state(app);
    ready(address);
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
