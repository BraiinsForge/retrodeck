use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("retrodeck-uploader-test-{}", token().unwrap()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn app(&self) -> App {
        App {
            config: Config {
                data_root: self.0.clone(),
                restart_command: None,
                ..Config::default()
            },
            auth: Mutex::new(Auth::default()),
            storage: Mutex::new(()),
            login: Arc::new(Semaphore::new(1)),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn palette() -> HashMap<String, String> {
    PALETTE_ROLES
        .iter()
        .map(|n| ((*n).to_owned(), "#123ABC".to_owned()))
        .collect()
}

#[test]
fn password_configuration_roundtrip_and_private_descriptor_checks() {
    let fixture = Fixture::new();
    let path = fixture.0.join("password.conf");
    let salt = [0x56; 16];
    let digest = pbkdf2_hmac_array::<Sha256, 32>(b"fixture-password", &salt, 100_000);
    let text = format!(
        "version=1\niterations=100000\nsalt={}\ndigest={}\n",
        STANDARD_NO_PAD.encode(salt),
        STANDARD_NO_PAD.encode(digest)
    );
    install(&path, text.as_bytes(), false).unwrap();
    let config = PasswordConfig::read(&path).unwrap();
    assert!(config.verify(b"fixture-password"));
    assert!(!config.verify(b"wrong"));
    assert!(!config.verify(&[b'x'; 129]));
    let link = fixture.0.join("password-link");
    symlink(&path, &link).unwrap();
    assert!(PasswordConfig::read(&link).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(PasswordConfig::read(&path).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    for bad in [
        text.replace("iterations=100000", "iterations=99999"),
        text.replace("digest=", "salt="),
        text.clone() + "extra\n",
        text.replace("version=1", "version=2"),
    ] {
        fs::write(&path, bad).unwrap();
        assert!(PasswordConfig::read(&path).is_err());
    }
}
#[test]
fn rom_validation_covers_supported_headers_checksums_and_limits() {
    assert!(validate_rom("nes", b"NES\x1a\0\0\0\0\0\0\0\0\0\0\0\0").is_ok());
    assert!(validate_rom("nes", b"NES\x1a").is_err());
    let mut gb = vec![0; 336];
    gb[260..308].copy_from_slice(&LOGO);
    gb[333] = gb[308..333]
        .iter()
        .fold(0u8, |n, b| n.wrapping_sub(*b).wrapping_sub(1));
    assert!(validate_rom("gb", &gb).is_ok());
    assert!(validate_rom("gbc", &gb).is_err());
    gb[323] = 128;
    gb[333] = gb[308..333]
        .iter()
        .fold(0u8, |n, b| n.wrapping_sub(*b).wrapping_sub(1));
    assert!(validate_rom("gbc", &gb).is_ok());
    gb[333] ^= 1;
    assert!(validate_rom("gb", &gb).is_err());
    for tap in [&[2, 0, 255, 255][..], &[2, 0, 128, 128, 2, 0, 42, 42][..]] {
        assert!(validate_rom("zx", tap).is_ok());
    }
    for tap in [
        &[2, 0, 1, 2][..],
        &[3, 0, 0, 0][..],
        &[2, 0, 0][..],
        &[2, 0, 0, 0, 0][..],
    ] {
        assert!(validate_rom("zx", tap).is_err());
    }
    assert!(validate_rom("gba", &[1]).is_ok());
    assert!(validate_rom("gba", &[]).is_err());
    assert!(validate_rom("bogus", &[1]).is_err());
}
#[test]
fn zip_decoding_rejects_paths_links_duplicates_damage_and_expansion() {
    let tap = [2, 0, 128, 128];
    let zipped = archive(&[("game.tap", &tap)]);
    assert_eq!(decode_upload("zx", "game.ZIP", &zipped).unwrap(), tap);
    for files in [
        vec![("../game.tap", &tap[..])],
        vec![("sub/game.tap", &tap[..])],
        vec![("sub\\game.tap", &tap[..])],
        vec![("game.nes", &tap[..])],
        vec![("one.tap", &tap[..]), ("two.tap", &tap[..])],
    ] {
        assert!(decode_upload("zx", "bad.zip", &archive(&files)).is_err());
    }
    let mut link = ZipWriter::new(Cursor::new(Vec::new()));
    link.add_symlink("link.tap", "../target", SimpleFileOptions::default())
        .unwrap();
    assert!(decode_upload("zx", "link.zip", &link.finish().unwrap().into_inner()).is_err());
    let oversized = vec![0; system("nes").unwrap().maximum + 1];
    assert!(decode_upload("nes", "big.zip", &archive(&[("big.nes", &oversized)])).is_err());
    assert!(decode_upload("zx", "broken.zip", &zipped[..zipped.len() - 10]).is_err());
    let mut trailing = zipped.clone();
    trailing.extend_from_slice(b"trailing data");
    assert!(decode_upload("zx", "trailing.zip", &trailing).is_err());
    let mut prefixed = b"prefix".to_vec();
    prefixed.extend_from_slice(&zipped);
    assert!(decode_upload("zx", "prefixed.zip", &prefixed).is_err());
    let mut corrupted = zipped;
    let payload = 30
        + u16::from_le_bytes([corrupted[26], corrupted[27]]) as usize
        + u16::from_le_bytes([corrupted[28], corrupted[29]]) as usize;
    corrupted[payload] ^= 64;
    assert!(decode_upload("zx", "crc.zip", &corrupted).is_err());
    assert!(decode_upload("gba", "big.gba", &vec![0; UPLOAD_LIMIT + 1]).is_err());
}
#[test]
fn atomic_storage_does_not_overwrite_or_follow_links_and_is_private() {
    let fixture = Fixture::new();
    let path = fixture.0.join("roms/zx/test.tap");
    install(&path, b"old", false).unwrap();
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert!(install(&path, b"new", false).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"old");
    let link = fixture.0.join("roms/zx/link.tap");
    symlink(&path, &link).unwrap();
    assert!(install(&link, b"unsafe", false).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"old");
    symlink(fixture.0.join("roms/zx"), fixture.0.join("bad-dir")).unwrap();
    assert!(install(&fixture.0.join("bad-dir/unsafe.tap"), b"data", false).is_err());
    assert!(!fixture.0.join("roms/zx/unsafe.tap").exists());
    assert!(fs::read_dir(path.parent().unwrap()).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".uploader-")
    }));
}
#[test]
fn uploaded_catalog_is_sorted_exactly_and_rom_is_rolled_back_on_catalog_failure() {
    let fixture = Fixture::new();
    let app = fixture.app();
    install(&app.config.path("nes-deck/menu/games.tsv"), b"", true).unwrap();
    add_rom(&app, "zx", "Zulu", "z.tap", &[2, 0, 0, 0]).unwrap();
    add_rom(&app, "zx", "Alpha", "a.tap", &[2, 0, 1, 1]).unwrap();
    let entries = read_catalog(&app.config.path("nes-deck/uploads/games.tsv"), false).unwrap();
    assert_eq!(entries[0][0], "upload-zx-alpha");
    assert_eq!(entries[0][1], "Alpha");
    assert_eq!(entries[0][4], "#AF87D7");
    assert_eq!(entries[1][1], "Zulu");
    assert!(add_rom(&app, "zx", "Alpha", "a.tap", &[2, 0, 9, 9]).is_err());
    assert_eq!(
        fs::read(app.config.path("roms/zx/alpha.tap")).unwrap(),
        [2, 0, 1, 1]
    );
    let catalog = app.config.path("nes-deck/uploads/games.tsv");
    fs::remove_file(&catalog).unwrap();
    fs::create_dir(&catalog).unwrap();
    assert!(add_rom(&app, "zx", "Rollback", "b.tap", &[2, 0, 0, 0]).is_err());
    assert!(!app.config.path("roms/zx/rollback.tap").exists());
}
#[test]
fn palette_schema_and_legacy_icon_roundtrip_are_bounded_and_complete() {
    let p = palette();
    assert_eq!(
        parse_appearance_override(&encode_palette(&p))
            .unwrap()
            .into_iter()
            .collect::<HashMap<_, _>>(),
        p
    );
    let tsv = PALETTE_ROLES
        .iter()
        .map(|n| format!("{n}\t#123abc\n"))
        .collect::<String>();
    assert_eq!(parse_palette_tsv(&tsv).unwrap(), p);
    assert_eq!(
        parse_palette_tsv(&(tsv.clone() + "settings-icon\t09\n")).unwrap(),
        p
    );
    assert!(parse_palette_tsv(&(tsv.clone() + "settings-icon\t../x\n")).is_err());
    assert!(parse_palette_tsv(&(tsv.clone() + "accent\t#112233\n")).is_err());
    assert!(parse_palette_tsv("background\t#123ABC\n").is_err());
    let fixture = Fixture::new();
    let config = fixture.app().config;
    install(
        &config.path("nes-deck/menu/palette.tsv"),
        tsv.as_bytes(),
        true,
    )
    .unwrap();
    install(
        &config.path("nes-deck/state/dashboard-palette.sexp"),
        b"#.(error \"never evaluate\")",
        true,
    )
    .unwrap();
    assert_eq!(current_palette(&config).unwrap(), p);
}
#[test]
fn session_expiry_cookie_tokens_and_login_blocking_use_monotonic_time() {
    let fixture = Fixture::new();
    let app = fixture.app();
    let s = app.create_session().unwrap();
    assert_eq!(s.id.len(), 43);
    assert_eq!(s.csrf.len(), 43);
    assert_ne!(s.id, s.csrf);
    let mut headers = HeaderMap::new();
    headers.insert(
        "cookie",
        format!("other=x; deck_rom_session={}", s.id)
            .parse()
            .unwrap(),
    );
    assert!(app.session(&headers).is_some());
    app.auth
        .lock()
        .unwrap()
        .sessions
        .get_mut(&s.id)
        .unwrap()
        .expires = Instant::now();
    assert!(app.session(&headers).is_none());
    let host = Ipv4Addr::LOCALHOST;
    for _ in 0..4 {
        app.record_login(host, false);
        assert!(app.blocked(host).is_none());
    }
    app.record_login(host, false);
    assert_eq!(app.blocked(host), Some(300));
    app.record_login(host, true);
    assert!(app.blocked(host).is_none());
    assert!(same_token("abc", "abc"));
    assert!(!same_token("abc", "abd"));
    assert!(!same_token("abc", "ab"));
}
#[test]
fn request_authority_and_origin_are_exact_ipv4_and_port_matches() {
    let mut headers = HeaderMap::new();
    headers.insert("host", "127.0.0.1:8080".parse().unwrap());
    assert!(host_allowed(&headers, 8080));
    assert!(same_origin(&headers));
    for host in [
        "deck.local:8080",
        "127.0.0.1",
        "127.000.0.1:8080",
        "[::1]:8080",
        "127.0.0.1:08080",
        "127.0.0.1:8081",
    ] {
        headers.insert("host", host.parse().unwrap());
        assert!(!host_allowed(&headers, 8080));
    }
    headers.insert("host", "127.0.0.1:8080".parse().unwrap());
    for origin in ["null", "http://127.0.0.1:8080"] {
        headers.insert("origin", origin.parse().unwrap());
        assert!(same_origin(&headers));
    }
    for origin in [
        "http://127.0.0.1:8081",
        "http://example.com",
        "https://127.0.0.1:8080",
    ] {
        headers.insert("origin", origin.parse().unwrap());
        assert!(!same_origin(&headers));
    }
    for title in ["", " trailing", "trailing ", "a\tb", "a\u{85}b"] {
        assert!(!title_valid(title));
    }
    assert!(title_valid("Pokémon"));
    assert_eq!(slugify("Long Title / test"), "long-title-test");
}
