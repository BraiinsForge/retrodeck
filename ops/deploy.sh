#!/usr/bin/env bash

# Build and install the complete persistent Retro Deck payload.

set -euo pipefail
export LC_ALL=C

usage() {
  echo "Usage: $0 [--config PATH] [--check-config] [root@DECK-IP]" >&2
  exit 2
}

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
activate_script=$script_dir/deploy/activate.sh
config_library=$script_dir/lib/deck-config.sh
config=$repo_root/deck.conf
check_config=0
target_override=

while [[ $# -gt 0 ]]; do
  case $1 in
    --config)
      [[ $# -ge 2 ]] || usage
      config=$2
      shift 2
      ;;
    --check-config)
      check_config=1
      shift
      ;;
    -* ) usage ;;
    *)
      [[ -z $target_override ]] || usage
      target_override=$1
      shift
      ;;
  esac
done

[[ -f $config_library && ! -L $config_library ]] || {
  echo "Deck configuration library is missing or unsafe: $config_library" >&2
  exit 1
}
# shellcheck source=ops/lib/deck-config.sh disable=SC1091
source "$config_library"
deck_config_load "$config" "$target_override"
target=$DECK_SSH_TARGET
wireguard_address=$DECK_WIREGUARD_ADDRESS
uploader_password=$ROM_UPLOADER_PASSWORD

if [[ $check_config -eq 1 ]]; then
  echo "Deck configuration is valid for $target at $wireguard_address"
  exit 0
fi

[[ -f $activate_script && ! -L $activate_script ]] || {
  echo "Remote activation script is missing or unsafe: $activate_script" >&2
  exit 1
}

for command in nix ssh tar gzip sha256sum; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "Missing required command: $command" >&2
    exit 1
  }
done

cd "$repo_root"

work=$(mktemp -d "${TMPDIR:-/tmp}/nes-deck-deploy.XXXXXX")
trap 'rm -rf "$work"' EXIT INT TERM HUP
payload=$work/payload

build_flake() {
  local attribute=$1
  nix build --no-link --print-out-paths "$attribute" | tail -n 1
}

echo "Building static ARM payloads..."
nes=$(build_flake .#nes-deck)
gb=$(build_flake .#gb-deck)
zx=$(build_flake .#zx-deck)
gba=$(build_flake .#gba-deck)
native=$(build_flake .#retrodeck-native)
fbterm=$(build_flake .#fbterm-deck)
rlwrap=$(build_flake .#rlwrap-deck)
lua=$(build_flake .#lua-deck)
python=$(build_flake .#python-deck)
chibi=$(build_flake .#chibi-deck)
uploader_libraries=$(build_flake .#uploader-lisp-libraries)
runtime_licenses=$(build_flake .#runtime-licenses)
ecl=$(build_flake .#ecl-arm-network)

mkdir -p \
  "$payload/nes-deck/menu" \
  "$payload/nes-deck/games" \
  "$payload/nes-deck/lisp" \
  "$payload/nes-deck/langs/chibi/lib" \
  "$payload/nes-deck/licenses" \
  "$payload/nes-deck/terminal/fonts" \
  "$payload/nes-deck/terminal/keymaps" \
  "$payload/nes-deck/uploader" \
  "$payload/bmc-widgets/retro-deck/bin" \
  "$payload/nes-deck/ecl" \
  "$payload/chiptunes" \
  "$payload/roms" \
  "$payload/usr/bin" \
  "$payload/usr/sbin" \
  "$payload/etc/init.d"

cp "$nes/bin/nes-deck" "$payload/nes-deck/nes-deck"
cp "$gb/bin/gb-deck" "$payload/nes-deck/gb-deck"
cp "$zx/bin/zx-deck" "$payload/nes-deck/zx-deck"
cp "$gba/bin/gba-deck" "$payload/nes-deck/gba-deck"
ln -sfn retrodeck-native "$payload/nes-deck/ten-seconds-deck"
ln -sfn ../retrodeck-native "$payload/nes-deck/menu/deck-menu"
cp "$native/bin/retrodeck-native" "$payload/nes-deck/retrodeck-native"
cp lisp/startup.lisp lisp/ui.lisp lisp/timer.lisp lisp/policy.lisp \
  lisp/chiptune.lisp lisp/process.lisp lisp/settings.lisp lisp/wifi.lisp \
  lisp/credits.lisp \
  lisp/dashboard.lisp "$payload/nes-deck/lisp/"
ln -sfn retrodeck-native "$payload/nes-deck/chiptune-deck"
cp lisp/uploader.lisp lisp/uploader-paper.css lisp/uploader-palette.js \
  deploy/uploader/run.lisp "$payload/nes-deck/uploader/"
cp -a "$uploader_libraries/share/common-lisp/source" \
  "$payload/nes-deck/uploader/lisp-libraries"
chmod -R u+w "$payload/nes-deck/uploader/lisp-libraries"
printf '%s\n' "$uploader_password" |
  nix shell nixpkgs#python3 -c python3 ops/lib/set-uploader-password.py \
    "$payload/nes-deck/uploader/password.conf"
cp "$lua/bin/lua" "$payload/nes-deck/langs/lua"
cp "$python/bin/python" "$payload/nes-deck/langs/python"
cp "$chibi/bin/chibi-scheme" \
  "$payload/nes-deck/langs/chibi/chibi-scheme"
cp -a "$chibi/share/chibi/." "$payload/nes-deck/langs/chibi/lib/"
cp -a "$ecl/bin" "$ecl/lib" "$payload/nes-deck/ecl/"

cp "$fbterm/bin/fbterm" "$fbterm/bin/loadkeys" \
  "$payload/nes-deck/terminal/"
cp "$rlwrap/bin/rlwrap" "$payload/nes-deck/terminal/"
cp "$fbterm/share/retro-deck/fonts/DejaVuSansMono.ttf" \
  "$payload/nes-deck/terminal/fonts/"
cp -a "$fbterm/share/retro-deck/keymaps/." \
  "$payload/nes-deck/terminal/keymaps/"
cp deploy/terminal/fonts.conf deploy/terminal/retro-terminal \
  "$payload/nes-deck/terminal/"

cp deploy/menu/games.sexp deploy/menu/games.tsv deploy/menu/credits.tsv \
  deploy/menu/palette.tsv \
  deploy/menu/ASSETS.md \
  deploy/menu/compile-catalog.lisp deploy/menu/deck-menu-launcher \
  deploy/menu/fetch-covers "$payload/nes-deck/menu/"
cp assets/settings-cog/gear-knekko-09.png \
  "$payload/nes-deck/menu/settings-icon.png"
cp deploy/widget/manifest.json \
  "$payload/bmc-widgets/retro-deck/manifest.json"
cp deploy/widget/retro-deck \
  "$payload/bmc-widgets/retro-deck/bin/retro-deck"
chmod 0755 "$payload/bmc-widgets/retro-deck/bin/retro-deck"
cp deploy/ecl "$payload/usr/bin/ecl"
cp ops/deck-wifi/deck-wifi-profile-add \
  "$payload/usr/sbin/deck-wifi-profile-add"
cp ops/deck-wifi/deck-wifi-select ops/deck-wifi/deck-wifi-watch \
  "$payload/usr/sbin/"
cp ops/deck-wifi/deck-wifi.init "$payload/etc/init.d/deck-wifi"
cp deploy/menu/nes-deck.init "$payload/etc/init.d/nes-deck"
cp deploy/menu/nes-deck-swap.init "$payload/etc/init.d/nes-deck-swap"
mkdir -p "$payload/etc/hotplug.d/usb"
cp deploy/menu/nes-deck-keyboard.hotplug \
  "$payload/etc/hotplug.d/usb/90-nes-deck-keyboard"
cp deploy/menu/deck-keyboard-quirks \
  "$payload/usr/sbin/deck-keyboard-quirks"
cp deploy/uploader/nes-deck-uploader.init \
  "$payload/etc/init.d/nes-deck-uploader"
cp ops/lib/install-bmc-scene.py "$payload/install-bmc-scene.py"

for result in "$nes" "$gb" "$zx" "$fbterm" "$rlwrap" "$lua" \
              "$python" "$chibi" "$runtime_licenses" "$ecl"; do
  if [[ -d $result/share/licenses ]]; then
    cp -a "$result/share/licenses/." "$payload/nes-deck/licenses/"
  fi
done

if [[ -d chiptunes ]]; then
  find chiptunes -maxdepth 1 -type f \( -name '*.ogg' -o -name '*.ay' -o \
    -name '*.gbs' -o -name '*.gym' -o -name '*.hes' -o -name '*.kss' -o \
    -name '*.nsf' -o -name '*.nsfe' -o -name '*.sap' -o -name '*.spc' -o \
    -name '*.vgm' -o -name '*.vgz' \) -exec cp {} "$payload/chiptunes/" \;
fi

for system in nes gb gbc zx gba; do
  mkdir -p "$payload/roms/$system"
  if [[ -d roms/$system ]]; then
    cp -a "roms/$system/." "$payload/roms/$system/"
  fi
done

find "$payload/nes-deck" -type f \( \
  -name 'nes-deck' -o -name 'gb-deck' -o -name 'zx-deck' -o -name 'gba-deck' -o \
  -name 'ten-seconds-deck' -o \
  -name 'chiptune-deck' -o -name 'retrodeck-native' -o \
  -name 'deck-menu' -o \
  -name 'deck-menu-launcher' -o -name 'fetch-covers' -o \
  -name 'retro-terminal' -o -name 'fbterm' -o -name 'loadkeys' -o \
  -name 'rlwrap' -o \
  -name 'lua' -o -name 'python' -o -name 'chibi-scheme' -o \
  -name 'ecl.bin' \) -exec chmod 0700 {} +
chmod 0600 "$payload/nes-deck/lisp/startup.lisp" \
  "$payload/nes-deck/lisp/ui.lisp" \
  "$payload/nes-deck/lisp/timer.lisp" \
  "$payload/nes-deck/lisp/policy.lisp" \
  "$payload/nes-deck/lisp/chiptune.lisp" \
  "$payload/nes-deck/lisp/process.lisp" \
  "$payload/nes-deck/lisp/settings.lisp" \
  "$payload/nes-deck/lisp/wifi.lisp" \
  "$payload/nes-deck/lisp/credits.lisp" \
  "$payload/nes-deck/lisp/dashboard.lisp"
chmod 0600 "$payload/nes-deck/uploader/password.conf" \
  "$payload/nes-deck/uploader/uploader.lisp" \
  "$payload/nes-deck/uploader/uploader-paper.css" \
  "$payload/nes-deck/uploader/uploader-palette.js" \
  "$payload/nes-deck/uploader/run.lisp"
chmod 0700 "$payload/usr/bin/ecl" \
  "$payload/usr/sbin/deck-keyboard-quirks" \
  "$payload/usr/sbin/deck-wifi-profile-add" \
  "$payload/usr/sbin/deck-wifi-select" \
  "$payload/usr/sbin/deck-wifi-watch" \
  "$payload/etc/hotplug.d/usb/90-nes-deck-keyboard" \
  "$payload/etc/init.d/deck-wifi" \
  "$payload/etc/init.d/nes-deck" \
  "$payload/etc/init.d/nes-deck-swap" \
  "$payload/etc/init.d/nes-deck-uploader"

remote_stage=/mnt/data/.nes-deck-deploy-$$
echo "Uploading staged payload to $target..."
# The generated staging path is intentionally expanded on the trusted client.
# shellcheck disable=SC2029
ssh "$target" "rm -rf '$remote_stage'; mkdir -p '$remote_stage'; chmod 700 '$remote_stage'"
# shellcheck disable=SC2029
if ! tar -C "$payload" -czf - . | ssh "$target" \
  "gzip -dc | tar -C '$remote_stage' -xf -"; then
  # The generated staging path is constrained above and contains only digits.
  # shellcheck disable=SC2029
  ssh "$target" "rm -rf '$remote_stage'" >/dev/null 2>&1 || :
  echo "Payload transfer failed; removed the remote staging directory" >&2
  exit 1
fi

echo "Validating and activating payload..."
ssh "$target" sh -s -- "$remote_stage" <"$activate_script"

echo
echo "Deployment complete. Verify with:"
printf '  %q --config %q %q\n' "$script_dir/check-deck.sh" "$config" "$target"
