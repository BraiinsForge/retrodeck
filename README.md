# Retro Deck

A touch-first game and program launcher for the Braiins Forge Deck. Runs as a
native fullscreen BMC widget, with a direct-framebuffer fallback.

Includes NES, Game Boy, Game Boy Color and ZX Spectrum emulators; DOOM;
10 Seconds; a terminal; chiptunes; and ECL Common Lisp, Chibi Scheme, Lua and
MicroPython REPLs. The dashboard and services are standalone Rust applications.
Two THEGamepad controllers provide Player 1 and Player 2 input.

## Install

Use a Linux host with Nix flakes and SSH. The Deck needs root SSH access,
mounted `/mnt/data`, stable power, and stock BMC with an initialized Nix profile
and version-2 `/etc/bmc/config.json`. See [BMC setup](BUILD.md#stock-bmc-installation).

```sh
git clone git@github.com:BraiinsForge/retrodeck.git
cd retrodeck
./ops/configure-deck.sh
./ops/provision-deck.sh --check
./ops/provision-deck.sh
```

Configure the SSH target, WireGuard address and uploader password in the private,
Git-ignored `deck.conf`. Provisioning prepares networking and installs Retro Deck.
Verify the Deck's SSH host key first; check provisioning options with `--help`.

Update and check an existing installation:

```sh
./ops/deploy.sh --config deck.conf
./ops/check-deck.sh --config deck.conf
```

Deployment preserves scenes, instance identities, saves and user content.
For a Deck without BMC, provision with `--network-only`, then deploy with
`--presentation framebuffer`. Build and test commands are in [BUILD.md](BUILD.md).

## Use

- Tap a tab or use L/R to change sections. Select a card with Left/Right and A.
- Keyboard: arrows to move, Enter to launch, Escape to return, Tab/Shift-Tab to
  change sections. The gear or controller Select opens settings.
- Hold the touchscreen for two seconds to leave an emulator or terminal.
  Touch does not emulate game controls.
- NES/GB/GBC: A/X and B/Y, Start and Back. Keyboard fallback: arrows/WASD,
  Space/Z/J, Shift/X/K, Enter and Control.
- DOOM: D-pad moves/turns, L/R strafes, A fires, B uses, X/Y changes weapons,
  Start opens the menu and Back opens the map. Use a keyboard for save names.
- ZX: Player 1 uses Kempston, Player 2 Sinclair 2. A/X fires, Back opens the
  Spectrum keyboard, L is Enter and R is Space.
- Keep identical controllers in the same USB ports to preserve player order.
- In the chiptune player, A pauses, Left/Right changes files, L/R changes
  subsongs, Up/Down adjusts volume, Start changes mode and B returns.
- REPL files persist under `/mnt/data/langs/{lua,lisp,python,scheme}`.
  `RETRO_DECK_REDUCED_MOTION=1` replaces the animated credits with a static sheet.

## Games and music

Upload a raw ROM or single-ROM ZIP at `http://<DECK_ADDRESS>:8080`, using the
configured password. See [uploader limits and authentication](deploy/uploader/README.md).

Stage owner-supplied console images in `roms/<system>/`; installed paths are
`/mnt/data/roms/<system>/`. Keep media, firmware and their checksums local.
See [ROM and save layout](roms/README.md) and [DOOM IWADs](deploy/doom/README.md).
DOOM saves belong in `/mnt/data/doom`.

Put music in `/mnt/data/chiptunes`. See the [included CC0 tracks](chiptunes/README.md).

## Development

- [Builds and tests](BUILD.md)
- [BMC contracts and upstream requirements beyond 60 fps](docs/upstream-rust-migration.md#upstream-requirements-beyond-60-fps)
- [Installed layout and catalog schema](deploy/menu/README.md)
- [Hardware and recovery notes](DECK_NOTES.md)
- [Third-party licenses and pinned sources](THIRD_PARTY.md)

Installed binaries include their required license texts. Owner-supplied game
images retain their original licensing.
