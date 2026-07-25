#!/usr/bin/env bash

# Build and run every host-side regression test without polluting the repo.

set -euo pipefail

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
cd "$repo_root"
unset RETRO_DECK_VOLUME_PERCENT

cargo=${CARGO:-cargo}
sbcl=${SBCL:-sbcl}
for command in "$cargo" "$sbcl" nix; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "Missing required command: $command" >&2
    exit 1
  }
done

work=$(mktemp -d "${TMPDIR:-/tmp}/nes-deck-tests.XXXXXX")
trap 'rm -rf "$work"' EXIT INT TERM HUP

CARGO_TARGET_DIR="$work/cargo-target" \
  "$cargo" test --manifest-path native/Cargo.toml --locked --lib
"$sbcl" --noinform --disable-debugger --script tests/lisp_policy_test.lisp

tests/rom_library_test.sh
tests/catalog_test.sh
tests/licenses_test.sh
tests/fetch_covers_test.sh
tests/settings_icons_test.sh
tests/deploy_config_test.sh
tests/deploy_activation_test.sh
tests/check_deck_test.sh
tests/provision_config_test.sh
tests/deck_wifi_profile_add_test.sh
tests/deck_wifi_select_test.sh
tests/deck_keyboard_quirks_test.sh
tests/retro_terminal_test.sh
tests/nes_deck_swap_test.sh

echo "All host tests passed."
