# ROM intake web server

The standalone Rust uploader (`native/src/uploader/`, `rom-uploader`)
accepts owner-supplied ROMs at the Deck's active Wi-Fi or WireGuard IPv4
address on port 8080. It listens on every IPv4 interface; requests must use an
IPv4 literal host on port 8080, and form origins must match that exact
address. The `nes-deck-uploader` init script runs
`/mnt/data/nes-deck/uploader/rom-uploader`. Password configuration checks and
verification use `rom-uploader --check-uploader-password-config PATH` and
`rom-uploader --verify-uploader-password PATH` (password on stdin);
`ops/lib/set-uploader-password.py` writes the matching configuration at
deployment time.

The service uses a PBKDF2-HMAC-SHA256 password record, bounded login attempts,
eight-hour same-site sessions, CSRF tokens, strict origin and host checks, and
a 12 MiB request ceiling. It validates raw ROMs or a ZIP containing exactly
one matching ROM. Existing files are never replaced. Successful files go to
`/mnt/data/roms/<system>/`, and the private supplemental catalog is written
atomically to `/mnt/data/nes-deck/uploads/games.tsv` before the dashboard is
restarted.

The authenticated page also edits all semantic dashboard colors as full
`#RRGGBB` values. It writes one complete, strictly validated version-2
S-expression to `/mnt/data/nes-deck/state/dashboard-palette.sexp` and restarts
the dashboard. Existing version-3 overrides remain readable: their colors are
preserved and their retired settings-icon choice is ignored. Built-in defaults
remain available when the optional override is malformed.

Run `cargo build --manifest-path native/Cargo.toml --bin rom-uploader`, then
`tests/uploader-rust-http.sh native/target/debug/rom-uploader` for the HTTP
contract tests. The ARM/QEMU `uploader-password-smoke` check exercises the
configuration parser and password verifier.
