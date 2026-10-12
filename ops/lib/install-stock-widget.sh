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
  [ -d "$root/run/current-profile/lib/bmc-widgets" ] &&
  [ -f "$root/etc/bmc/config.json" ] &&
  [ ! -L "$root/etc/bmc/config.json" ] || {
  echo 'Stock BMC compositor, active Nix profile, and /etc/bmc/config.json are required' >&2
  exit 1
}
for command in nix-store bmc-nix-cli; do
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
[ -f "$root$widget/lib/bmc-widgets/retro-deck/manifest.json" ] &&
  [ -x "$root$widget/lib/bmc-widgets/retro-deck/bin/retro-deck" ] || {
  echo 'Imported widget package is incomplete' >&2
  exit 1
}
# Merge from current, preserving every other installed package. Stock hooks
# activate the profile and notify the compositor about the new widget.
bmc-nix-cli add-packages --name retro-deck-widget --version 1.0.0 \
  --store-path "$widget"
[ -f "$root/run/current-profile/lib/bmc-widgets/retro-deck/manifest.json" ] || {
  echo 'Retro Deck widget did not enter the active BMC profile' >&2
  exit 1
}

# Commit the scene only after the package is in the active profile. The caller
# stops the compositor before installation and starts it after this write.
"$stage/nes-deck/langs/python" "$stage/install-bmc-scene.py" \
  "$root/etc/bmc/config.json"
