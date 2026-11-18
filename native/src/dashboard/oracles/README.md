# Dashboard render references

Menu fixtures record the former Lisp renderer before its removal. Replay the
menu transcripts through the shared Rust canvas, then compare full-frame RGB565
hashes with the current renderer in `menu_tests.rs`. `parity_tests.rs` checks
settings snapshots and the former Lisp Wi-Fi and credits frames.

- `menu-cases.txt`: console, empty, card-count, tab and status cases.
- `menu-raster.txt`: PNG, custom-cover and tint cases.
- `credits.tsv`: credits from `d202a1f`, frozen independently of production credits.

Render the current settings fixture with
`cargo run --manifest-path native/Cargo.toml --example settings-frame -- OUTPUT.png`.

Menu records are `C` (clear), `R` (rectangle), `G` (glyph with numeric code point)
and `P` (raster kind, position, dimensions and tint); `CASE` separates frames.
The original renderer was loaded in SBCL with primitive-recording stubs, using
synthetic catalogs and PNGs. These are test data, not an executable Lisp runtime.
