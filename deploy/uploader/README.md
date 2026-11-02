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
a 36 MiB request ceiling. It accepts NES, Game Boy, Game Boy Color, Game Boy
Advance and ZX Spectrum images, raw or in a ZIP containing exactly one matching
ROM. ROM limits are 8 MiB, or 32 MiB for GBA; the uploaded file limit is 34 MiB.
Existing ROM files are never replaced. Successful files go to
`/mnt/data/roms/<system>/`. The private supplemental catalog is replaced by rename
at `/mnt/data/nes-deck/uploads/games.tsv` before the dashboard is restarted.

ROM and catalog publication are separate operations, not a crash-atomic pair.
A directory-sync failure after publication produces a warning about uncertain
power-loss durability. After catalog publication the referenced ROM is retained;
a pre-publication catalog failure rolls back the new ROM and reports any failed
rollback.

The authenticated page also edits all semantic dashboard colors as full
`#RRGGBB` values. It writes one complete, strictly validated version-2
S-expression to `/mnt/data/nes-deck/state/dashboard-palette.sexp` and restarts
the dashboard. Existing version-3 overrides remain readable: their colors are
preserved and their retired settings-icon choice is ignored. Built-in defaults
remain available when the optional override is malformed.
The authenticated `POST /palette/reset` endpoint removes the override; it requires
the same origin, session and CSRF checks as palette updates.

Run `cargo build --manifest-path native/Cargo.toml --bin rom-uploader`, then
`tests/uploader-rust-http.sh native/target/debug/rom-uploader` for the HTTP
contract tests. The ARM/QEMU `uploader-password-smoke` check exercises the
configuration parser and password verifier.
