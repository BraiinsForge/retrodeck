//! Legacy drawing transcripts replayed through the shared RGB565 canvas.
use super::*;
use crate::canvas;

fn reference_frame(transcript: &str) -> u64 {
    for line in transcript.lines() {
        let mut fields = line.split_whitespace();
        let kind = fields.next().unwrap();
        let values: Vec<u32> = fields.map(|v| v.parse().unwrap()).collect();
        match (kind, values.as_slice()) {
            ("C", [color]) => canvas::clear(*color),
            ("R", [x, y, w, h, color]) => canvas::fill_rect(*x as i32, *y as i32, *w, *h, *color),
            ("G", [x, y, code, scale, color]) => {
                canvas::draw_glyph(*x as i32, *y as i32, *code as u8, *scale, *color)
            }
            ("P", [kind, x, y, w, h, color]) => {
                let path = artwork();
                let handle = if *kind == 0 {
                    canvas::load_png_raster(&path, 23, 23).unwrap()
                } else {
                    canvas::load_cover_raster(&path, *color).unwrap()
                };
                canvas::draw_raster(handle, *x as i32, *y as i32, *w, *h).unwrap();
            }
            _ => panic!("invalid reference operation: {line}"),
        }
    }
    canvas::rgb565_hash()
}

#[test]
fn builtin_carousel_matches_legacy_frames() {
    let references = [
        include_str!("oracles/menu-0.txt"),
        include_str!("oracles/menu-1.txt"),
        include_str!("oracles/menu-2.txt"),
        include_str!("oracles/menu-3.txt"),
        include_str!("oracles/menu-4.txt"),
        include_str!("oracles/menu-5.txt"),
        include_str!("oracles/menu-6.txt"),
        include_str!("oracles/menu-7.txt"),
    ];
    let games = super::super::policy::builtins();
    for (position, reference) in references.into_iter().enumerate() {
        let expected = reference_frame(reference);
        MenuRenderer::default().render(&games, System::Deck, position, "STATE READY");
        assert_eq!(
            canvas::rgb565_hash(),
            expected,
            "builtin carousel position {position}"
        );
    }
}

fn fixture_games() -> Vec<Game> {
    [
        ("alpha", System::Nes, 0x5f87ff),
        ("beta", System::Nes, 0xafafff),
        ("gamma", System::Nes, 0x87afaf),
        ("delta", System::Nes, 0xd75f5f),
        ("epsilon", System::Gb, 0x87af87),
    ]
    .into_iter()
    .map(|(id, system, color)| Game {
        id: id.into(),
        title: id.to_uppercase(),
        system,
        rom: format!("/r/{id}"),
        color,
        cover: None,
        terminal_mode: None,
    })
    .collect()
}

#[test]
fn console_carousels_tabs_and_status_match_legacy_frames() {
    let fixtures = [
        (0, System::Nes, 0),
        (1, System::Nes, 0),
        (2, System::Nes, 1),
        (3, System::Nes, 2),
        (4, System::Nes, 0),
        (4, System::Nes, 3),
        (5, System::Gb, 0),
        (5, System::Nes, 1),
    ];
    let long_status = "THIS MENU STATUS FIXTURE IS INTENTIONALLY LONG ENOUGH TO USE THE SMALL FOOTER SCALE ACROSS THE COMPLETE DISPLAY";
    let games = fixture_games();
    let transcript = include_str!("oracles/menu-cases.txt");
    let references: Vec<_> = transcript.split("CASE ").skip(1).collect();
    assert_eq!(references.len(), fixtures.len());
    for (index, ((count, system, position), reference)) in
        fixtures.into_iter().zip(references).enumerate()
    {
        let (_, operations) = reference.split_once('\n').unwrap();
        let expected = reference_frame(operations);
        let status = if index == 7 {
            long_status
        } else {
            "STATE READY"
        };
        MenuRenderer::default().render(&games[..count], system, position, status);
        assert_eq!(canvas::rgb565_hash(), expected, "console fixture {index}");
    }
}

fn artwork() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/settings-cog/gear-knekko-09.png")
}

#[test]
fn png_settings_custom_covers_and_tint_cache_match_legacy_frames() {
    let references: Vec<_> = include_str!("oracles/menu-raster.txt")
        .split("CASE ")
        .skip(1)
        .collect();
    let mut game = fixture_games().remove(0);
    game.cover = Some(artwork());
    let renderer = MenuRenderer::new("/missing/covers", Some(artwork()));
    for (index, reference) in references.into_iter().enumerate() {
        game.color = if index == 2 { 0xd75f5f } else { 0x5f87ff };
        let (_, operations) = reference.split_once('\n').unwrap();
        let expected = reference_frame(operations);
        let fallback = MenuRenderer::new("/missing/covers", None);
        let selected = if index == 0 { &fallback } else { &renderer };
        selected.render(std::slice::from_ref(&game), System::Nes, 0, "STATE READY");
        assert_eq!(canvas::rgb565_hash(), expected, "raster fixture {index}");
        selected.render(std::slice::from_ref(&game), System::Nes, 0, "STATE READY");
        assert_eq!(canvas::rgb565_hash(), expected);
    }
    assert_eq!(renderer.rasters.borrow().len(), 3);
}

#[test]
fn failed_cover_load_is_cached_until_renderer_replacement() {
    let directory = crate::test_support::fixture_directory("dashboard-cover");
    let path = directory.join("cover.png");
    let mut game = fixture_games().remove(0);
    game.cover = Some(path.clone());
    let renderer = MenuRenderer::default();
    renderer.render(std::slice::from_ref(&game), System::Nes, 0, "");
    let missing = canvas::rgb565_hash();
    std::fs::copy(artwork(), &path).unwrap();
    renderer.render(std::slice::from_ref(&game), System::Nes, 0, "");
    assert_eq!(canvas::rgb565_hash(), missing);
    MenuRenderer::default().render(&[game], System::Nes, 0, "");
    assert_ne!(canvas::rgb565_hash(), missing);
}
