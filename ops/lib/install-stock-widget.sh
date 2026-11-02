#!/bin/sh

# Stock BMC profile installation. ROOT is used only by offline fixtures.
set -eu
[ "$#" -ge 2 ] && [ "$#" -le 3 ] || {
  echo 'Usage: install-stock-widget.sh check|install STAGE [ROOT]' >&2
  exit 2
}
mode=$1
stage=$2
root=${3:-}
case "$mode" in check|install) ;; *) exit 2 ;; esac
PATH=$root/run/current-profile/bin:$PATH
export PATH

[ -x "$root/etc/init.d/bmc-compositor" ] &&
  [ -r "$root/run/current-profile/manifest" ] &&
  [ -f "$root/etc/bmc/config.json" ] &&
  [ ! -L "$root/etc/bmc/config.json" ] || {
  echo 'Stock BMC compositor, active Nix profile, and /etc/bmc/config.json are required' >&2
  exit 1
}
for command in nix-store bmc-nix-cli readlink; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "Stock BMC command is missing: $command" >&2
    exit 1
  }
done
[ -s "$stage/widget.nar" ] && [ -f "$stage/widget-store-path" ] || {
  echo 'Staged stock widget Nix package is missing' >&2
  exit 1
}
widget=$(cat "$stage/widget-store-path")
# A single store path, never shell syntax or another CLI option.
case "$widget" in /nix/store/*-bmc-widget-retro-deck-1.0.0) ;; *)
  echo 'Invalid widget store path' >&2; exit 1 ;; esac
case "$widget" in *[!a-zA-Z0-9/._-]*)
  echo 'Invalid widget store path characters' >&2; exit 1 ;; esac
store_name=${widget#/nix/store/}
case "$store_name" in */*) echo 'Widget must be a direct Nix store entry' >&2; exit 1 ;; esac
"$stage/nes-deck/langs/python" "$stage/install-bmc-scene.py" \
  --check "$root/etc/bmc/config.json"
bmc-nix-cli is-initialized || {
  echo 'Stock BMC Nix store is not initialized and mounted' >&2
  exit 1
}
[ "$mode" = install ] || exit 0

nix-store --import <"$stage/widget.nar"
package=$root$widget/lib/bmc-widgets/retro-deck
for executable in bin/retro-deck bin/deck-menu bin/compile-catalog libexec/deck-menu-launcher; do
  [ -x "$package/$executable" ] || {
    echo "Imported widget executable is missing: $executable" >&2
    exit 1
  }
done
for resource in manifest.json share/menu/credits.tsv share/menu/palette.tsv share/menu/settings-icon.png; do
  [ -s "$package/$resource" ] || {
    echo "Imported widget resource is missing: $resource" >&2
    exit 1
  }
done
# Run the packaged ARM dashboard before changing the profile or scene. This
# catches corrupt exports, wrong-architecture builds and loader failures.
"$package/bin/deck-menu" --help >/dev/null

# Merge from current, preserving every other installed package. Stock hooks
# activate the profile and notify the compositor about the new widget.
bmc-nix-cli add-packages --base current --name retro-deck-widget --version 1.0.0 \
  --store-path "$widget"
active=$root/run/current-profile/lib/bmc-widgets/retro-deck
[ "$(readlink -f "$active/bin/retro-deck")" = \
  "$(readlink -f "$package/bin/retro-deck")" ] &&
  [ -f "$active/manifest.json" ] || {
  echo 'Requested Retro Deck package did not enter the active BMC profile' >&2
  exit 1
}

# Commit the scene only after the package is in the active profile. Activation
# may have started BMC through upstream hooks; the caller restarts it after this
# write so the process reads the newly persisted scene as well as the profile.
"$stage/nes-deck/langs/python" "$stage/install-bmc-scene.py" \
  "$root/etc/bmc/config.json"
