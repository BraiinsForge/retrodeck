#!/usr/bin/env bash
set -euo pipefail

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$repo_root/native/target}
work=$(mktemp -d "${TMPDIR:-/tmp}/retrodeck-hosts.XXXXXX")
trap 'rm -rf "$work"' EXIT

# Cargo's console binaries link engine archives supplied by Nix. These host
# unit tests need only the native library and the small memory ABI fixture.
"${CARGO:-cargo}" build --manifest-path native/Cargo.toml --locked --lib
cat > "$work/hosts.rs" <<EOF
#[allow(dead_code)]
#[path = "$repo_root/native/src/bin/retro-host/host.rs"]
mod retro_host;
#[allow(dead_code)]
#[path = "$repo_root/native/src/bin/doom-host/host.rs"]
mod doom_host;
EOF
"${RUSTC:-rustc}" --edition 2024 --test "$work/hosts.rs" \
  --extern "retrodeck_native=$CARGO_TARGET_DIR/debug/libretrodeck_native.rlib" \
  -L "dependency=$CARGO_TARGET_DIR/debug/deps" -o "$work/hosts"
"$work/hosts"

# Supply only the upstream timer declarations and exercise our actual backend.
cat > "$work/i_timer.h" <<'EOF'
#define TICRATE 35
int I_GetTime(void);
int I_GetTimeMS(void);
EOF
cat > "$work/doomtype.h" <<'EOF'
#ifndef TEST_DOOMTYPE_H
#define TEST_DOOMTYPE_H
typedef int boolean;
#endif
EOF
cat > "$work/timer.c" <<'EOF'
#include <assert.h>
#include <limits.h>
#include "i_timer.h"
static int milliseconds;
int retrodeck_doom_ticks(void) { return milliseconds; }
void retrodeck_doom_sleep(int ms) { (void) ms; }
int main(void) {
    const int inputs[] = {0, 1000, 61356675, 61356676, 61356677, 86400000, INT_MAX};
    const int expected[] = {0, 35, 2147483, 2147483, 2147483, 3024000, 75161927};
    for (unsigned i = 0; i < sizeof(inputs) / sizeof(inputs[0]); ++i) {
        milliseconds = inputs[i];
        assert(I_GetTime() == expected[i]);
        assert(I_GetTimeMS() == milliseconds);
    }
    return 0;
}
EOF
"${CC:-cc}" -std=c99 -Wall -Wextra -Werror -I "$work" \
  native/doom/i_timer_retrodeck.c "$work/timer.c" -o "$work/timer"
"$work/timer"
echo 'Native host and DOOM timer tests passed.'
