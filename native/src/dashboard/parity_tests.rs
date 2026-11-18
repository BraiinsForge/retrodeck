//! Baseline parity oracles for the Rust dashboard port.
//!
//! Legacy constants are copied from `tests/native_ecl_smoke.lisp.in`; settings
//! snapshots reflect the current controls. Keep these tests serial: the
//! canvas is thread-local, but its raster stores are shared by the
//! render sequence within a test.

#[cfg(test)]
mod tests {
    use super::super::{credits, menu, policy, settings, ui, wifi};
    use crate::canvas;

    // Animated frames use subpixel coverage. The static hashes are the ECL oracle.
    const CREDITS_HASHES: &[(bool, i64, u64)] = &[
        (false, 0, 0x94ebf079be6e596b),
        (false, 2_000, 0x9565b54afcc5a92d),
        (false, 20_000, 0x870d475b1e2256a5),
        (false, 600_000_000, 0xd5e26939d81659ff),
        (true, 0, 0xdc88adde04077b1b),
        (true, 60_000, 0xdc88adde04077b1b),
    ];

    fn fixture_credits() -> Vec<credits::Credit> {
        let text = include_str!("oracles/credits.tsv");
        credits::parse(text).expect("checked-in credits fixture must parse")
    }

    #[test]
    fn credits_crawl_matches_lisp_fixture_shape() {
        let credits = fixture_credits();
        assert_eq!(credits.len(), 33);
        assert_eq!(credits[0].project, "FCEUmm");
        assert_eq!(credits.last().unwrap().project, "OpenGameArt contributors");
        let crawl = credits::make_crawl(&credits);
        assert_eq!(crawl.lines.len(), 104);
        assert_eq!(crawl.static_lines.len(), 33);
        assert_eq!(crawl.content_height, 5556);
        assert_eq!(crawl.lines.first().unwrap().text, "RETRO DECK");
        assert_eq!(crawl.lines.first().unwrap().source_y, 0);
        assert_eq!(crawl.lines.first().unwrap().source_width, 236);
        let last = crawl.lines.last().unwrap();
        assert_eq!(
            (last.text.as_str(), last.source_y, last.source_width),
            ("THANK YOU", 5512, 212)
        );
        assert!(
            crawl
                .lines
                .iter()
                .all(|line| line.source_width <= 1040 && line.source_height == 28)
        );
    }

    #[test]
    fn credits_frame_hashes() {
        let mut page = credits::Credits::new(fixture_credits());
        for &(reduced, elapsed, expected) in CREDITS_HASHES {
            page.tick(elapsed);
            page.render(reduced, 0, 0xffffac, 0xeeeeee, 0x949594)
                .expect("credits render");
            assert_eq!(
                canvas::rgb565_hash(),
                expected,
                "credits frame at {elapsed} ms"
            );
        }
    }

    #[test]
    fn menu_geometry_matches_baseline_contract() {
        ui::install_palette(
            policy::DEFAULT_PALETTE_VALUES
                .iter()
                .map(|(n, c)| ((*n).into(), *c))
                .collect(),
        );
        let games = vec![
            policy::Game {
                id: "alpha".into(),
                title: "ALPHA".into(),
                system: policy::System::Nes,
                rom: "/r/alpha".into(),
                cover: None,
                color: 0x5f87ff,
                terminal_mode: None,
            },
            policy::Game {
                id: "beta".into(),
                title: "BETA".into(),
                system: policy::System::Nes,
                rom: "/r/beta".into(),
                cover: None,
                color: 0xafafff,
                terminal_mode: None,
            },
        ];
        let layout = menu::MenuRenderer::default().render(&games, policy::System::Nes, 1, "");
        assert_eq!(layout.credits, menu::Bounds::new(12, 412, 56, 56));
        assert_eq!(layout.settings, menu::Bounds::new(1212, 412, 56, 56));
        assert_eq!(layout.previous, Some(menu::Bounds::new(156, 232, 80, 100)));
        assert_eq!(layout.next, Some(menu::Bounds::new(1044, 232, 80, 100)));
        assert_eq!(
            layout.game_buttons,
            vec![
                menu::Bounds::new(406, 154, 216, 264),
                menu::Bounds::new(658, 154, 216, 264)
            ]
        );
        assert_eq!(layout.target_at(156, 232), Some(menu::Target::Previous));
        assert_eq!(layout.target_at(236, 232), None);
    }

    #[test]
    fn settings_and_wifi_geometry_match_lisp_oracle() {
        let state = settings::State::new(42, "us");
        let ops = settings::render_with_network(&state, &Default::default());
        assert!(matches!(
            ops.first(),
            Some(settings::DrawOp::Clear(0x000000))
        ));
        assert_eq!(
            settings::Layout::default().close,
            settings::Rect::new(1212, 12, 56, 56)
        );
        assert_eq!(
            settings::Layout::default().wifi,
            settings::Rect::new(926, 20, 262, 108)
        );

        let wifi_state = wifi::State::default();
        let wifi_ops = wifi::render_with_network(&wifi_state, &Default::default());
        assert!(matches!(
            wifi_ops.first(),
            Some(settings::DrawOp::Clear(0x000000))
        ));
        assert_eq!(
            wifi::target_at_mode(wifi::Mode::Alphabet, 18, 86),
            Some(wifi::Target::Key('q'))
        );
        assert_eq!(wifi::target_at_mode(wifi::Mode::Alphabet, 137, 147), None);
        assert_eq!(wifi::rows(wifi::Mode::Alphabet).len(), 4);
    }

    fn fixture_network() -> crate::network::NetworkStatus {
        crate::network::NetworkStatus {
            ssid: "net1".into(),
            wlan_ipv4: "10.249.110.248".into(),
            wireguard_ipv4: "10.0.0.10".into(),
            selector: "CONNECTED".into(),
        }
    }

    fn fixture_palette() {
        ui::install_palette(
            policy::DEFAULT_PALETTE_VALUES
                .iter()
                .map(|(name, color)| ((*name).into(), *color))
                .collect(),
        );
    }

    #[test]
    fn settings_frame_hashes_match_reference() {
        use settings::Target::*;
        fixture_palette();
        let network = fixture_network();
        let fixtures = [
            (42, "us", VolumeDown, "", 0x3cbe278b5a0957b7),
            (0, "us", VolumeUp, "", 0xfc35836e10af4537),
            (42, "cz", Keymap, "", 0xe66395d275349597),
            (42, "us", Wifi, "", 0xef904ba047d3fac7),
            (42, "us", Terminal, "", 0xa02266e45ba33aff),
            (
                42,
                "us",
                VolumeDown,
                "VOLUME SAVED; CONFIRMATION TONE FAILED",
                0x04d9b229305443f7,
            ),
            (
                42,
                "us",
                VolumeDown,
                "THIS SETTINGS STATUS FIXTURE IS INTENTIONALLY LONG ENOUGH TO USE THE SMALL FOOTER SCALE ACROSS THE COMPLETE DISPLAY",
                0x73f6b2e8d928aaf7,
            ),
        ];
        let observed = fixtures.map(|(volume, keymap, selected, status, _)| {
            let mut state = settings::State::new(volume, keymap);
            state.selected = selected;
            state.status = status.into();
            settings::render_frame(&state, &network);
            (selected, canvas::rgb565_hash())
        });
        let expected = fixtures.map(|(_, _, selected, _, hash)| (selected, hash));
        assert_eq!(observed, expected);
    }

    #[test]
    fn wifi_frame_hashes_match_reference() {
        fixture_palette();
        let network = fixture_network();
        let fixtures = [
            (
                "",
                "",
                wifi::Field::Ssid,
                false,
                wifi::Mode::Alphabet,
                0xd6be2f43c4faf0e6,
            ),
            (
                "",
                "",
                wifi::Field::Ssid,
                true,
                wifi::Mode::Alphabet,
                0x7682dc83b0062730,
            ),
            (
                "NETWORK",
                "password",
                wifi::Field::Password,
                true,
                wifi::Mode::Alphabet,
                0xa5c18f4c41654088,
            ),
            (
                "NETWORK",
                "password",
                wifi::Field::Password,
                false,
                wifi::Mode::Symbols,
                0xf919741f85fe2c31,
            ),
        ];
        for (ssid, password, field, uppercase, mode, expected) in fixtures {
            let mut state = wifi::State::new(ssid, password, field);
            state.uppercase = uppercase;
            state.mode = mode;
            wifi::render_frame(&state, &network);
            assert_eq!(canvas::rgb565_hash(), expected, "wifi {field:?}/{mode:?}");
        }
    }

    #[test]
    fn shared_palette_is_explicit_and_not_an_implicit_oracle() {
        ui::install_palette(
            policy::DEFAULT_PALETTE_VALUES
                .iter()
                .map(|(n, c)| ((*n).into(), *c))
                .collect(),
        );
        assert_eq!(ui::color("accent"), 0xfe6c27);
        assert_eq!(ui::color("title"), 0xffffaf);
    }
}
