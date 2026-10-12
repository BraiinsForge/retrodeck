#!/usr/bin/env bash

set -euo pipefail

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
fixture=$(mktemp -d /tmp/retro-deck-catalog-test.XXXXXX)
trap 'rm -rf "$fixture"' EXIT INT TERM HUP

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

# Build only the std-only catalog module, independently of the ECL UI host.
# Callers with a packaged binary can supply RETRODECK_CATALOG_COMPILER.
compiler=${RETRODECK_CATALOG_COMPILER:-}
if [ -z "$compiler" ]; then
  cat >"$fixture/Cargo.toml" <<EOF
[package]
name = "retrodeck-catalog-test"
version = "0.0.0"
edition = "2024"
[lib]
name = "retrodeck_native"
path = "lib.rs"
[[bin]]
name = "compile-catalog"
path = "$root/native/src/bin/compile-catalog.rs"
EOF
  printf '#[path = "%s/native/src/catalog.rs"]\npub mod catalog;\n' "$root" \
    >"$fixture/lib.rs"
  cargo test --offline --manifest-path "$fixture/Cargo.toml" --lib
  cargo build --offline --manifest-path "$fixture/Cargo.toml" --bin compile-catalog
  compiler=$fixture/target/debug/compile-catalog
fi

compile_catalog() {
  "$compiler" "$1" "$2" "$3" "$fixture/no-override.sexp"
}

compile_catalog "$root/deploy/menu/games.sexp" \
  "$fixture/games.tsv" "$fixture/palette.tsv" >/dev/null
cmp -s "$fixture/games.tsv" "$root/deploy/menu/games.tsv" ||
  fail 'checked-in games.tsv differs from games.sexp'
cmp -s "$fixture/palette.tsv" "$root/deploy/menu/palette.tsv" ||
  fail 'checked-in palette.tsv differs from games.sexp'

sed 's#/mnt/data/nes-deck/games/ten-seconds#/tmp/ten-seconds#' \
  "$root/deploy/menu/games.sexp" >"$fixture/unsafe.sexp"
if compile_catalog "$fixture/unsafe.sexp" "$fixture/unsafe.tsv" \
  "$fixture/unsafe-palette.tsv" >/dev/null 2>&1; then
  fail 'Deck application outside its installed directory was accepted'
fi

cp "$fixture/games.tsv" "$fixture/old-games.tsv"
cp "$fixture/palette.tsv" "$fixture/old-palette.tsv"
printf '#.(error "reader evaluation")\n' >"$fixture/reader-eval.sexp"
if compile_catalog "$fixture/reader-eval.sexp" "$fixture/games.tsv" \
  "$fixture/palette.tsv" >/dev/null 2>&1; then
  fail 'reader evaluation syntax was accepted'
fi
cmp -s "$fixture/games.tsv" "$fixture/old-games.tsv" ||
  fail 'invalid catalog replaced the last valid games output'
cmp -s "$fixture/palette.tsv" "$fixture/old-palette.tsv" ||
  fail 'invalid catalog replaced the last valid palette output'
if "$compiler" >/dev/null 2>&1; then
  fail 'missing compiler arguments were accepted'
fi

# Exercise only the launcher's catalog-selection block, with local fixtures.
# The launcher itself also performs device setup and is not executed here.
sed -n '/^manifest=\$FALLBACK_MANIFEST$/,/^if \[ -s "\$UPLOAD_MANIFEST"/p' \
  "$root/deploy/menu/deck-menu-launcher" | sed '$d' \
  >"$fixture/catalog-selection.sh"
[ -s "$fixture/catalog-selection.sh" ] || fail 'catalog selection block missing'
printf '#!/bin/sh\ntest -s "$2"\n' >"$fixture/menu-validator"
chmod +x "$fixture/menu-validator"
cp "$root/deploy/menu/games.sexp" "$fixture/current.sexp"
select_catalog() (
  CATALOG=$fixture/current.sexp
  COMPILER=${1:-$compiler}
  MENU=$fixture/menu-validator
  GENERATED_MANIFEST=$fixture/cached-games.tsv
  GENERATED_PALETTE=$fixture/cached-palette.tsv
  FALLBACK_MANIFEST=$root/deploy/menu/games.tsv
  FALLBACK_PALETTE=$root/deploy/menu/palette.tsv
  PALETTE_OVERRIDE=$fixture/no-override.sexp
  LOG_FILE=$fixture/catalog-selection.log
  log_message() { :; }
  . "$fixture/catalog-selection.sh"
  printf '%s\t%s\n' "$manifest" "$palette"
)
fallback=$(printf '%s\t%s' "$root/deploy/menu/games.tsv" "$root/deploy/menu/palette.tsv")
generated=$(printf '%s\t%s' "$fixture/cached-games.tsv" "$fixture/cached-palette.tsv")
[ "$(select_catalog "$fixture/no-compiler")" = "$fallback" ] ||
  fail 'missing compiler did not select fallback TSVs'
[ "$(select_catalog)" = "$generated" ] ||
  fail 'Rust compilation did not select generated TSVs'
[ "$(select_catalog "$fixture/no-compiler")" = "$generated" ] ||
  fail 'valid cache required a compiler'
cp "$fixture/cached-games.tsv" "$fixture/cached-games-before.tsv"
cp "$fixture/cached-palette.tsv" "$fixture/cached-palette-before.tsv"
touch -t 200001010000 "$fixture/cached-games.tsv" "$fixture/cached-palette.tsv"
printf '(:version 6)\n' >"$fixture/current.sexp"
[ "$(select_catalog)" = "$fallback" ] ||
  fail 'failed compilation did not select fallback TSVs'
cmp -s "$fixture/cached-games.tsv" "$fixture/cached-games-before.tsv" ||
  fail 'failed compilation changed cached games'
cmp -s "$fixture/cached-palette.tsv" "$fixture/cached-palette-before.tsv" ||
  fail 'failed compilation changed cached palette'

echo 'catalog-test: OK'
