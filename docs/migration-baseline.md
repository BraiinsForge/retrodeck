# Historical parity fixture provenance

Current build instructions are in [BUILD.md](../BUILD.md); current contracts
and acceptance limits are in [upstream-rust-migration.md](upstream-rust-migration.md).

## Frame conventions

Frame hashes below are FNV-1a over complete 1280x480 logical RGB565 images,
with expected values supplied by C++ reference renderers.
Historical fbdev captures contained 1,638,400 stride-aware bytes.
All 614,400 active pixels were unrotated using:
`physical-row = 1279 - logical-x`, `physical-column = logical-y`.

## Dashboard fixtures

Recorded physical fbdev captures matched the bitmap, static dashboard, raster,
initial NES, credits, settings and Wi-Fi frames below.
Navigation rows preserve C++ frame hashes and matching Lisp state/render/cue
traces; physical panel navigation was not observed.

| Reference fixture | RGB565 FNV-1a |
| --- | --- |
| Bitmap UI: `menu_ui.cpp` 5x7 font, cut panel, outline, fitted text, `?` fallback | `414079453e1344d5` |
| Nine-game dashboard: six tabs, shifted NES carousel, third game selected, long title, indicators/status | `65b48f5f3b66d535` |
| Raster dashboard: approved 23x23 gear PNG as settings icon and selected cover | `5c932dc59681241e` |
| Initial NES touch fixture | `0ef078d4dc7a53bd` |
| Navigation trace: first Next | `9f7ec7647982e7bd` |
| Navigation trace: second Next | `de67cf4c35ff2b4d` |
| Navigation trace: GAME BOY tab | `4e9094bcf7a7f9e5` |
| Animated credits, 0 ms | `94ebf079be6e596b` |
| Animated credits, 2,000 ms | `1f14f6b786549363` |
| Animated credits, 20,000 ms | `6267b51f6f787c83` |
| Animated credits, 600,000,000 ms | `f62d9d0147c7461a` |
| Reduced-motion credits, both 0 and 60,000 ms | `9a44bcef4a13dde3` |
| Settings: volume 42, brightness 60, US, volume-down selected | `46d1527abb9f2bcb` |
| Settings: muted, brightness 60, US, volume-up selected | `c2c55ee7eb47608b` |
| Settings: volume 42, maximum brightness, US, brightness-up selected | `6e348df7ca27725f` |
| Settings: volume 42, brightness 60, CZ, keymap selected | `99ed5871b55b5f6b` |
| Settings: volume 42, brightness 60, US, Wi-Fi selected | `65f7d573c69bccbb` |
| Settings: volume 42, brightness 60, US, terminal selected | `9cabcc3df5188ce3` |
| Settings: confirmation-tone failure status | `05a5652bb03e0b8b` |
| Settings: reduced-scale long status | `773a6a165672bd8b` |
| Wi-Fi: empty lowercase alphabet | `d6be2f43c4faf0e6` |
| Wi-Fi: empty uppercase alphabet | `7682dc83b0062730` |
| Wi-Fi: uppercase password field, `NETWORK`, masked `password` | `a5c18f4c41654088` |
| Wi-Fi: symbol keyboard with the same populated fields | `f919741f85fe2c31` |

## 10 Seconds frames

The reference layout scales 624x224 to 1280x480 with nearest-neighbor sampling.
Historical ARM/ECL canvas comparisons matched these independent C++ frames;
those comparisons did not establish physical input or timing acceptance.

| Reference fixture | RGB565 FNV-1a |
| --- | --- |
| Ready, 00.00 | `562ba370a3b4da05` |
| Running, 01.23 | `802225e33aea53b5` |
| Exact, 10.00 | `171e4a78d40d458d` |
| Late, 10.03 | `0171d3938f410bd5` |
| Capped, 99.99 | `d8877f2ba172c415` |

## Historical audio fingerprints

- Timer three-note cue: C++ waveform match, 5,732 samples,
  FNV-1a `b572c4d4420310d4`.
- Ogg preview: four-block hash `E58BD71B6BD7E1CD`; recorded matches through
  host, ARM/ECL and the BMC Wayland gameplay layer.
