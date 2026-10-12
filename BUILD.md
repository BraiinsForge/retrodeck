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

Install Nix with flakes enabled. Host tests require Rust, Bash, curl, zip,
Python 3, and coreutils.

On Debian or Ubuntu:

```sh
sudo apt-get install \
  cargo rustc curl zip python3 coreutils
```

Then clone the private repository:

```sh
git clone git@github.com:BraiinsForge/retrodeck.git
cd retrodeck
```

The first Nix build downloads the pinned cross toolchain and may take several
minutes. Later builds reuse the Nix store.

The dashboard (`deck-menu`), uploader (`rom-uploader`), timer, chiptune
player, and catalog compiler are standalone Rust executables. The console emulators are Rust libretro hosts
(`native/src/bin/retro-host/`) with the
pinned FCEUmm, Gambatte, Fuse, and gpSP cores statically linked, one binary per
console. The `libretro-host-smoke` flake check runs a representative ARM host
headless under QEMU against a reproducibly fetched free test input and pins its video hash.

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
nix build --no-link --print-out-paths .#deck-menu
nix build --no-link --print-out-paths .#rom-uploader
nix build --no-link --print-out-paths .#ten-seconds-deck
nix build --no-link --print-out-paths .#chiptune-deck
nix build --no-link --print-out-paths .#compile-catalog
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
| `deck-menu` | `bin/deck-menu`, standalone Rust dashboard |
| `rom-uploader` | `bin/rom-uploader`, standalone Rust HTTP service |
| `ten-seconds-deck` | `bin/ten-seconds-deck`, standalone Rust |
| `chiptune-deck` | `bin/chiptune-deck`, standalone Rust |
| `compile-catalog` | `bin/compile-catalog`, standalone Rust |
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

### Standalone applications

The dashboard, uploader, timer, chiptune player, and catalog compiler are Rust
executables. Shared rendering, audio, controller, process, and widget mechanisms
live in `native/src/`. The dashboard owns UI state and managed-child recovery;
the uploader owns its HTTP routes, authentication, ROM intake, and palette edits.
The ECL package is the separate Common Lisp language app.

Build and inspect the complete deployable matrix with:

```sh
tests/verify-arm-builds.sh
```

The matrix checks static ARM executables and empty runtime reference closures,
then runs application CLI and configuration probes under QEMU. It includes the
emulator and DOOM frame checks, uploader password validation, and network-enabled
ECL interpreter smoke test. Deployment runs these parsers against staged data
before stopping the installed services.

## Run the host test suite

The test runner compiles into a temporary directory and leaves the worktree
clean:

```sh
tests/run-host-tests.sh
```

It covers dashboard state and menu-sound policy, the Rust menu-tone renderer, NES mixer, APU
noise, SRAM codec, controller and keyboard input, dashboard geometry and
behavior, ROM catalog, cover cache, Wi-Fi profile helper, rlwrap-backed
terminal lifecycle, shared framebuffer/audio runtime, and timer
configuration.

`tests/uploader-rust-http.sh` exercises the standalone uploader's full HTTP
contract against a host binary. ARM/QEMU checks validate the deployed password
parser and verifier.

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
├── native/                     Rust applications and shared native mechanisms
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
├── roms/                       local-only ROM staging
├── terminal/                   vendored fbterm source and provenance
├── tests/                      host regression suite
├── flake.nix                   pinned cross-build definitions
└── README.md                   deployment and operation guide
```

The exact on-device file contract and strict catalog schema are documented in
[deploy/menu/README.md](deploy/menu/README.md).
