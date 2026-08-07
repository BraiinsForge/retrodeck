# Build and test Retro Deck

Retro Deck uses Nix for reproducible ARMv7 hard-float builds. The generated
executables are static and run on the Deck's OpenWrt userspace without copying
a Nix store closure to the device.

For ordinary installation, create the private setup configuration and use the
complete deployment command in [README.md](README.md):

```sh
./ops/configure-deck.sh
./ops/deploy.sh
```

This document covers individual builds, verification, tests, and platform
details for development.

## Prerequisites

Install Nix with flakes enabled. Host tests also require Rust and Common
Lisp compilers.

On Debian or Ubuntu:

```sh
sudo apt-get install \
  cargo rustc sbcl
```

Then clone the private repository:

```sh
git clone git@github.com:BraiinsForge/retrodeck.git
cd retrodeck
```

The first Nix build downloads the pinned cross toolchain and may take several
minutes. Later builds reuse the Nix store.

The dashboard, timer, and chiptune player are startup-loaded Common Lisp
served by `retrodeck-native`; deployment installs them as `deck-menu`,
`ten-seconds-deck`, and `chiptune-deck` symlinks to that binary. The console
emulators are Rust libretro hosts (`native/src/bin/retro-host/`) with the
pinned FCEUmm, Gambatte, Fuse, and gpSP cores statically linked, one binary per
console. The `libretro-host-smoke` flake check runs each emulator headless
under QEMU against a tracked ROM and pins its 120-frame video hash.

DOOM follows the same shape without being a libretro core. `doom-deck` links
the pinned fbDOOM fork as a static archive (`doomLib`) into the Rust host in
`native/src/bin/doom-host/`. The fork's framebuffer, tty-keyboard, and timer
backends are excluded from that archive and replaced by the platform layer in
`native/doom/`, which routes video, controllers, audio, and the clock through
the same host modules the emulators use. Music needs three pieces fbDOOM
carries no copy of — an OPL emulator, a MIDI reader, and a MUS converter — so
those are vendored from the pinned Chocolate Doom release, with a Retro Deck
OPL driver in place of the SDL one. The `doom-host-smoke` check runs DOOM
headless against the pinned Freedoom IWAD for 600 frames and pins the video
hash, the synthetic clock, the sound-effect count, and the presence of OPL
output. It runs 600 frames rather than 120 because the title screen has to
give way to demo playback before a weapon fires.

Determinism in that check comes from `native/doom/i_timer_retrodeck.c`, which
derives DOOM's clock from the presented frame count whenever
`RETRO_DECK_TEST_FRAMES` is set. Without it, `I_GetTime` would read the wall
clock and no frame hash could be pinned.

## Build packages individually

Use `--no-link` to avoid leaving `result-*` symlinks in the repository:

```sh
nix build --no-link --print-out-paths .#retrodeck-native
nix build --no-link --print-out-paths .#nes-deck
nix build --no-link --print-out-paths .#gb-deck
nix build --no-link --print-out-paths .#zx-deck
nix build --no-link --print-out-paths .#gba-deck
nix build --no-link --print-out-paths .#doom-deck
nix build --no-link --print-out-paths .#fbterm-deck
nix build --no-link --print-out-paths .#rlwrap-deck
nix build --no-link --print-out-paths .#lua-deck
nix build --no-link --print-out-paths .#python-deck
nix build --no-link --print-out-paths .#chibi-deck
nix build --no-link --print-out-paths .#runtime-licenses
nix build --no-link --print-out-paths -f nix/ecl-arm-static.nix
```

| Package | Main output |
| --- | --- |
| `retrodeck-native` | `bin/retrodeck-native` |
| `nes-deck` | `bin/nes-deck` |
| `gb-deck` | `bin/gb-deck` |
| `zx-deck` | `bin/zx-deck` |
| `gba-deck` | `bin/gba-deck` |
| `fbterm-deck` | `bin/{fbterm,loadkeys}` plus font and keymaps |
| `rlwrap-deck` | `bin/rlwrap` |
| `lua-deck` | `bin/lua` |
| `python-deck` | `bin/python` |
| `chibi-deck` | `bin/chibi-scheme` plus Scheme modules |
| `runtime-licenses` | Shared runtime and asset notices |
| ECL expression | `bin/ecl.bin`, runtime library, and notices |

### Native orchestrator

`native/` contains `retrodeck-native`, the static ARM Rust/ECL host. It boots
ECL, registers the small `RETRODECK.NATIVE` interface, loads Common Lisp, and
uses the integer returned by `RETRODECK:MAIN` as its process status. The nix
build compiles the whole Lisp tree to native code (`nix/lisp-image.nix`, the
compiler-enabled ECL running under qemu-arm) and links it in, so the deployed
binary initializes the compiled image instead of loading sources. One alternate
startup path may still be supplied for development and smoke tests, and
setting `RETRO_DECK_LISP_SOURCE` forces the sibling
`lisp/startup.lisp` source tree (falling back to
`/mnt/data/nes-deck/lisp/startup.lisp`) — both load bytecode-compiled sources
exactly as before.

`lisp/startup.lisp` validates the native ABI, then loads `ui.lisp`,
`policy.lisp`, `chiptune.lisp`, `process.lisp`, `settings.lisp`, `wifi.lisp`,
`credits.lisp`, `dashboard.lisp`, and `timer.lisp`. These editable files own
bitmap UI, chiptune composition, and 10 Seconds composition and policy, systems,
labels, colors, applications,
exact game/reboot/terminal executable routes,
ordered arguments and environment, launch/return statuses and sequencing,
timing, settings and Wi-Fi editor state and actions, exact inherited and
persistent volume policy, exact brightness hardware and state policy, exact
terminal keymap state policy, Wi-Fi profile validation, helper request bytes,
completion statuses and cues, credits content and sequencing, dashboard geometry,
touch policy, keyboard and THEGamepad mapping, modal command priority,
controller burst recovery, input scan timing, and the Wi-Fi selector status path.
Startup finally loads an optional `local.lisp` beside them for device-local
overrides without a Rust
rebuild; the compiled image honors the same overlay after it initializes.
Deployment updates the ten standard Lisp files but leaves an
existing `local.lisp` untouched.

Native ABI 22 retains the widget-side Wayland and direct-fbdev primitives and
adds exact-display widget and gameplay Wayland opens plus only narrow canvas,
raster, projected-text, evdev, regular-file, network, state-file, control-file,
helper-process, audio, Ogg Vorbis decode, managed-child, terminal-process,
aggregate-input, process-shutdown, and monotonic-clock mechanisms for Lisp to
orchestrate. Audio now accepts a bounded
one-to-three-note Lisp sequence without changing the existing two-note menu
primitive, and the clock returns `CLOCK_MONOTONIC` nanoseconds as four portable
16-bit words for Lisp reconstruction. The Ogg boundary keeps one decoder handle,
returns title, artist, and duration metadata, and produces one exact 735-frame
little-endian stereo PCM block per Lisp step; Lisp owns filename fallback,
labels, rendering, rewind policy, and eventual track sequencing. The
exact-display primitive connects an
absolute socket path directly or a relative name below absolute
`XDG_RUNTIME_DIR` through `wayland-client`; the existing environment-based open
remains unchanged for explicit mode. A widget-presented child receives a
Wayland connection opened by the registered dashboard process, so BMC keeps the
normal `deck_widget_v1` identity after the child takes ownership of rendering
and touch. Gameplay fills that 1280x480 widget black and centers its exact
integer-scaled frame inside the 16-pixel safe area. Canvas colors take the
original RGB565 quantization path before XRGB presentation. Lisp snapshots
the policy-named
`WAYLAND_DISPLAY`, owns Wayland-first selection and fbdev fallback, and tracks
adoption and cleanup ownership. The bounded state-file mechanism distinguishes
missing files from exact bytes and performs private atomic replacement; Lisp
owns inherited volume parsing, defaults, legacy migration, canonical saves,
child reload policy,
and the exact missing/default/canonical terminal keymap state contract. The
control-file mechanism follows existing links, reads at most 63 exact bytes, and
writes bounded exact bytes to an existing path. Lisp owns brightness parsing,
rounding, startup normalization, hardware-before-state ordering, and settings
failure policy; Rust contains no backlight path or percentage knowledge. The
generic helper primitive starts one no-argument child with inherited environment
and output, writes and closes exact stdin bytes, waits, and reports the start,
input, wait, exit, or signal result. Lisp owns the Wi-Fi helper path, validation,
request construction, user-visible status, state update, and confirmation cue.
The generic managed-child primitive receives an executable, ordered arguments,
ordered environment pairs, a log label, and a touch-supervision flag. It closes
direct fbdev presentation, snapshots and restores the TTY, starts a separate
process group, polls every 40 ms, sends TERM for shutdown or the original
two-second touch hold, escalates the complete group to KILL after four seconds,
and reports started, touch, exit, signal, error, and shutdown fields. Lisp owns
the starting screen, one audio finish, control close, game/reboot dispatch,
forced control scan, presentation reopen, volume reload, final status, and fbdev
Deck-touch reconnect. Injected effect handlers retain priority. Per owner
approval, launch policy passes ROM paths directly to the established emulators
and lets them reject invalid content rather than duplicating their validation.
The network primitive reports the current `wlan0` SSID and IPv4 address, the
`wg0` IPv4 address, and the bounded Wi-Fi selector status while Lisp owns its
path, refresh timing, result shape, and rendering. One native
input poll waits on the selected Wayland or fbdev touchscreen first, then the
stable gamepad and keyboard descriptors, with one timeout. Ready controls are
read before touch; the fixed result reports queue counts, touch loss, control
rescan, Wayland shutdown, and process-signal shutdown while Lisp retains
mapping, arbitration, timing, and audio quarantine. A gamepad-only scan lets the
10 Seconds policy retain the original THEGamepad boundary without opening
keyboards. The control boundary otherwise keeps at most two exact THEGamepads
and four complete keyboards, decodes resynchronization and rising edges, and
returns raw reports for Lisp policy. The specialized terminal primitive uses the
same supervisor, accepts the exact keymap and mode boundary, restores console
state, accepts the original two-second touch return hold, and mirrors RGB565
console scanout only when the Wayland widget is
open. The fbdev path validates the device-reported 600x1280 RGB565 geometry and
stride, rotates the complete 1280x480 logical canvas in ordinary RAM, and
publishes finished rows. The Lisp runtime adapter now exposes initialization, a
single-iteration coordinator, and shutdown: Lisp reads the pre-poll clock,
runs recovery and timers, chooses the policy timeout, invokes the aggregate
native poll, dispatches the normalized snapshot, and preserves the combined
effect trace.

Build the host and run its focused mechanism tests with:

```sh
nix build --no-link --print-out-paths .#retrodeck-native
cargo test --manifest-path native/Cargo.toml --locked --lib
```

After installation, run the same ARM smoke test used during activation with:

```sh
ECLDIR=/mnt/data/nes-deck/ecl/lib/ecl/ \
  /mnt/data/nes-deck/retrodeck-native \
  /mnt/data/nes-deck/lisp/startup.lisp
```

Check that a package has no Nix runtime references before deploying it:

```sh
out=$(nix build --no-link --print-out-paths .#retrodeck-native | tail -n 1)
file "$out/bin/retrodeck-native"
test -z "$(nix-store -q --references "$out")"
```

The expected executable is a statically linked 32-bit ARM EABI5 binary.

Build and inspect the complete deployable matrix with:

```sh
tests/verify-arm-builds.sh
```

This rejects a missing executable, a non-ARM or dynamically linked binary, a
Nix store reference, an incomplete ECL or fbterm runtime, and a changed CC0
music payload. It includes `doom-deck` and its `doom-host-smoke` check. It also runs the ARM host under QEMU to exercise the embedded
ECL callbacks, including Wayland and fbdev boundaries, projected credits frames,
large elapsed times, helper stdin and process-result classification, and the
audio-worker lifecycle.

## Run the host test suite

The test runner compiles into a temporary directory and leaves the worktree
clean:

```sh
tests/run-host-tests.sh
```

It covers Lisp menu-sound policy, the Rust menu-tone renderer, NES mixer, APU
noise, SRAM codec, controller and keyboard input, dashboard geometry and
behavior, ROM catalog, cover cache, Wi-Fi profile helper, rlwrap-backed
terminal lifecycle, shared framebuffer/audio runtime, and timer
configuration.

The flake checks exercise the Lisp uploader's policy and full HTTP contract
on host SBCL and on the ARM network ECL under QEMU.

Run shell checks on deployment code with:

```sh
nix shell nixpkgs#shellcheck -c shellcheck -x \
  ops/lib/deck-config.sh ops/check-deck.sh ops/configure-deck.sh ops/deploy.sh \
  ops/deploy/activate.sh ops/provision-deck.sh \
  deploy/menu/nes-deck-swap.init \
  tests/run-host-tests.sh tests/deploy_config_test.sh \
  tests/deploy_activation_test.sh tests/check_deck_test.sh \
  tests/nes_deck_swap_test.sh \
  tests/verify-arm-builds.sh
```

## Validate language and music runtimes on a Deck

The deploy script performs basic Python, Scheme, and dashboard smoke tests
before stopping the running service. For focused checks against a staged
binary:

```sh
/mnt/data/nes-deck/langs/python -c 'print(6 * 7)'
CHIBI_MODULE_PATH=/mnt/data/nes-deck/langs/chibi/lib \
  /mnt/data/nes-deck/langs/chibi/chibi-scheme -q -p '(+ 20 22)'
```

## Platform details

The Deck CPU is ARMv7 Cortex-A7 hard-float. Its panel is a portrait 600x1280
RGB565 framebuffer used as a 1280x480 logical landscape display. The physical
pitch is 1280 bytes, including 80 bytes of padding per row, and only physical
columns 0 through 479 are visible. Code must use the stride reported by
`FBIOGET_FSCREENINFO`, not `xres * bytes_per_pixel`.

The menu fills the complete 1280x480 logical surface. Emulators and the
chiptune player use the shared scaler with a 16-pixel safe inset for the
rounded display. fbterm uses a 1248x448 viewport for the same reason. Every
frontend rejects unexpected geometry or color channel layouts rather than
guessing.

On BMC compositor installations, Retro Deck is a fullscreen scene widget.
The menu submits event-driven XRGB8888 shared-memory buffers through the Deck
widget protocol, so the compositor can move it during scene swipes. A launched
game receives a Wayland connection opened by that registered process, then
creates its own normal Deck widget surface through the inherited descriptor.
BMC therefore keeps the scene identity and directs touch to the game. The
emulator keeps its native frame clock and fills the 1280x480 widget black before
placing the integer-scaled game frame in its 16-pixel safe area. BMC's Smithay
renderer defaults to linear minification and magnification, which can still
soften pixel boundaries during the rotated composition pass. Apply the tracked
local patch before building a BMC image for Retro Deck:

```sh
ops/bmc/apply-local-patches.sh /root/bmc-main
nix build --no-link /root/bmc-main#deck-packages.core.pkg
```

The patch selects nearest-neighbor filtering for both directions. The script
is idempotent and refuses a source tree whose patch context does not match.
When the game exits, the dashboard resumes its normal widget surface and scene
swiping continues.

`ops/deploy.sh` installs the widget under `/mnt/data/bmc-widgets/retro-deck`.
If `bmc-compositor` is present, deployment stops it, adds one idempotent Retro
Deck scene to `/etc/bmc_config.json`, disables the legacy fbdev menu service,
enables a 64 MiB swapfile before BMC starts, and restarts the compositor. The
swap is needed because 128 MiB of the Deck's 256 MiB RAM is reserved for CMA;
without it, a stock BMC widget plus Retro Deck can trigger global OOM while
the first fullscreen SHM frame is faulted in. Existing swapfiles are left
untouched if they cannot be enabled. The original configuration is retained
once as `/etc/bmc_config.json.retro-deck.bak` before the first scene edit.

Audio uses `/dev/dsp` through the Deck's ALSA OSS bridge. All streams use
signed 16-bit little-endian samples. Emulator and chiptune streams are stereo;
the menu and timer cues are mono. FCEUmm reports 48 kHz. The OSS device stays at its
required nominal 48 kHz while the runtime resamples to the measured 47,328
frames/s application clock. Fuse, the timer, menu cues, and chiptunes
use 44.1 kHz. Gambatte produces 32,768 Hz and is explicitly resampled to the
Deck's verified 32 kHz OSS rate. Gain is applied in the native mixer because
the kernel OSS path bypasses ALSA userspace soft volume.

The framebuffer has no page-flip API. Frontends build complete frames in
cacheable memory and copy finished rows to fb0 to reduce tearing and protect
audio timing. `RETRO_DECK_RUNTIME_DIAGNOSTICS=1` logs 60-frame timing windows
from the shared libretro frontend.

## Source layout

```text
retrodeck/
├── chiptunes/                  CC0 seed tracks and provenance
├── deploy/
│   ├── menu/                   catalog, launcher, and procd service
│   ├── terminal/               fbterm wrapper, fontconfig, and keymaps
│   ├── uploader/               uploader service and credential plumbing
│   └── widget/                 BMC manifest, launcher, and scene installer
├── lisp/                       startup-loaded orchestration and policy
├── native/                     thin Rust/ECL native mechanism host
├── nix/                        ECL and runtime-specific Nix expressions
├── ops/
│   ├── bmc/                    external BMC patch application
│   ├── deck-menu/              cover cache tooling
│   ├── deck-wifi/              profile-only Wi-Fi helper
│   ├── deploy/                 validated on-Deck activation transaction
│   ├── lib/                    shared strict deployment configuration parser
│   ├── check-deck.sh           read-only installed health report
│   └── deploy.sh               local build, staging, and transfer
├── patches/                    pinned upstream fixes
├── protocol/                   Deck widget client protocol
├── roms/                       private canonical ROM library and checksums
├── terminal/                   vendored fbterm source and provenance
├── tests/                      host regression suite
├── flake.nix                   pinned cross-build definitions
└── README.md                   deployment and operation guide
```

The exact on-device file contract and strict catalog schema are documented in
[deploy/menu/README.md](deploy/menu/README.md).
