#!/usr/bin/env bash

# Match the deployment scripts literally, including their variable references.
# shellcheck disable=SC2016

set -euo pipefail

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
activation=$repo_root/ops/deploy/activate.sh
deployer=$repo_root/ops/deploy.sh

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

[[ -x $activation && ! -L $activation ]] ||
  fail 'activation script is not a regular executable'
sh -n "$activation"
bash -n "$deployer"

status=0
output=$(sh "$activation" 2>&1) || status=$?
[[ $status -eq 2 ]] || fail 'missing stage did not produce a usage error'
[[ $output == 'Usage: activate.sh STAGING-DIRECTORY' ]] ||
  fail 'activation usage text changed unexpectedly'

status=0
output=$(sh "$activation" /tmp/not-a-deck-stage 2>&1) || status=$?
[[ $status -eq 1 ]] || fail 'unsafe stage path was accepted'
[[ $output == 'Refusing unexpected staging path: /tmp/not-a-deck-stage' ]] ||
  fail 'unsafe stage path did not produce a specific error'

grep -Fq "activate_script=\$script_dir/deploy/activate.sh" "$deployer" ||
  fail 'deployer does not locate the activation script'
grep -Fq "ssh \"\$target\" sh -s -- \"\$remote_stage\" <\"\$activate_script\"" \
  "$deployer" || fail 'deployer does not stream the activation script'
grep -Fq "[[ -f \$activate_script && ! -L \$activate_script ]]" "$deployer" ||
  fail 'deployer does not validate the activation script'
grep -Fq 'cp "$chiptune/bin/chiptune-deck" "$payload/nes-deck/chiptune-deck"' \
  "$deployer" || fail 'deployer does not stage the chiptune executable'
grep -Fq 'snes=$(build_flake .#snes-deck)' "$deployer" ||
  fail 'deployer does not build the SNES host'
grep -Fq 'cp "$snes/bin/snes-deck" "$payload/nes-deck/snes-deck"' "$deployer" ||
  fail 'deployer does not stage the SNES host'
grep -Fq -e "-name 'snes-deck' -o" "$deployer" ||
  fail 'deployer does not make the SNES host executable'
grep -Fq '"$snes" "$doom"' "$deployer" ||
  fail 'deployer does not collect the SNES license notices'
grep -Fq 'nes gb gbc zx gba snes' "$deployer" ||
  fail 'deployer does not stage the SNES ROM directory'
grep -Fq 'cp -p "$stage/nes-deck/snes-deck" "$base/snes-deck"' "$activation" ||
  fail 'activation does not install the SNES host'
grep -Fq '"$stage/nes-deck/chiptune-deck" extra extra' "$activation" ||
  fail 'activation does not preflight the staged chiptune executable'
grep -Fq '[ "$chiptune_output" = "usage: chiptune-deck CHIPTUNE_DIRECTORY" ]' \
  "$activation" || fail 'activation does not check the chiptune usage result'
grep -Fq 'cp -p "$stage/nes-deck/chiptune-deck" "$base/chiptune-deck"' \
  "$activation" || fail 'activation does not install the chiptune executable'

grep -Fq 'doom=$(build_flake .#doom-deck)' "$deployer" ||
  fail 'deployer does not build the DOOM host'
grep -Fq 'cp "$doom/bin/doom-deck" "$payload/nes-deck/doom-deck"' "$deployer" ||
  fail 'deployer does not stage the DOOM host'
grep -Fq -e "-name 'doom-deck' -o" "$deployer" ||
  fail 'deployer does not make the DOOM host executable'
grep -Fq -e "find deploy/doom -maxdepth 1 -type f -name '*.wad'" "$deployer" ||
  fail 'deployer does not stage DOOM WADs'
grep -Fq '"$gba" "$snes" "$doom"' "$deployer" ||
  fail 'deployer does not collect the GBA, SNES and DOOM license notices'
grep -Fq 'ops/lib/extract-doom-cover.py' "$deployer" ||
  fail 'deployer does not render DOOM covers'
[[ -f $repo_root/ops/lib/extract-doom-cover.py ]] ||
  fail 'the DOOM cover renderer is missing'
python3 -c "import ast,sys; ast.parse(open(sys.argv[1]).read())" \
  "$repo_root/ops/lib/extract-doom-cover.py" ||
  fail 'the DOOM cover renderer does not parse'
# Merged rather than replaced, so the Libretro cover cache survives.
grep -Fq 'cp -Rp "$stage/nes-deck/covers/." "$base/covers/"' "$activation" ||
  fail 'activation does not merge the staged covers'
if grep -Fq 'rm -rf "$base/covers"' "$activation"; then
  fail 'activation deletes the persistent cover cache'
fi
grep -Fq 'nes-deck gb-deck zx-deck gba-deck snes-deck doom-deck tamagotchi-deck' \
  "$activation" || fail 'activation does not validate the staged Deck hosts'
grep -Fq 'cp -p "$stage/nes-deck/doom-deck" "$base/doom-deck"' "$activation" ||
  fail 'activation does not install the DOOM host'
grep -Fq 'cp -p "$stage/nes-deck/tamagotchi-deck" "$base/tamagotchi-deck"' \
  "$activation" || fail 'activation does not install the Tamagotchi host'
# DOOM's saves live outside the games directory because activation replaces
# that directory wholesale on every deployment.
grep -Fq '/mnt/data/doom' "$activation" ||
  fail 'activation does not create the persistent DOOM save directory'
grep -Fq '"$base/games.new/"' "$activation" ||
  fail 'activation no longer replaces the games directory; recheck DOOM saves'

if grep -Fq 'usr/bin/ecl' "$deployer"; then
  fail 'deployer stages a system-wide ECL command'
fi

# Exercise the legacy-wrapper cleanup without touching the host's /usr/bin.
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
cleanup=$(sed -n '/^legacy_ecl=/,/^fi$/p' "$activation" |
  sed 's@^legacy_ecl=.*@legacy_ecl=$1@')
wrapper=$fixture/ecl
cat > "$wrapper" <<'EOF'
#!/bin/sh

# Relocatable ECL 26.5.5 launcher for the Deck's /mnt/data payload.
ECL_ROOT=/mnt/data/nes-deck/ecl
export ECLDIR="$ECL_ROOT/lib/ecl/"
exec "$ECL_ROOT/bin/ecl.bin" "$@"
EOF
sh -ec "$cleanup" sh "$wrapper"
[[ ! -e $wrapper ]] || fail 'activation did not retire the former ECL wrapper'
printf 'owner-installed interpreter\n' > "$wrapper"
sh -ec "$cleanup" sh "$wrapper"
[[ -f $wrapper ]] || fail 'activation removed a different ECL interpreter'
printf 'deploy-activation-test: OK\n'
