# ROM intake web server

The Common Lisp uploader service (`lisp/uploader.lisp`, one Hunchentoot file)
accepts owner-supplied ROMs at the Deck's active Wi-Fi or WireGuard IPv4
address on port 8080. It listens on every IPv4 interface; requests must use an
IPv4 literal host on port 8080, and form origins must match that exact
address. The `nes-deck-uploader` init script runs it on the deployed
network-enabled ECL with `deploy/uploader/run.lisp` as the entry point and
the bundled `lisp-libraries` source tree on `CL_SOURCE_REGISTRY`. Password
verification calls `retrodeck-native --verify-uploader-password`;
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

The Lisp service is authoritative. `tests/uploader_lisp_test.lisp` and
`tests/uploader-http-smoke.sh` exercise its policy and full HTTP contract on
host SBCL and on the ARM network ECL under QEMU through the flake checks.
