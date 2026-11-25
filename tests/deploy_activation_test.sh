#!/usr/bin/env bash

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
