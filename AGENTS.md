# Repository Instructions

## Local game data

- Owner-supplied game images and firmware are local-only. Never commit them,
  their checksums, or root-level intake files.
- When local deployment needs a console image, place it under
  `roms/<system>/` using a lowercase kebab-case filename. Supported system
  directories are `nes`, `gb`, `gbc`, `gba`, and `zx`.
- Console catalog paths use `/mnt/data/roms/<system>/<filename>`. Deck
  applications are not ROMs and remain below `/mnt/data/nes-deck/games/`.
- Preserve `.sav`, `.rtc`, `.state`, and emulator configuration sidecars when
  moving a local Deck ROM. Keep each sidecar beside its ROM in the same system
  directory.

## Deck program data

- Local data for a Deck program belongs below `deploy/<program>/`, not
  `roms/`. `deploy/doom/` can hold a local DOOM IWAD for deployment.
- Owner-supplied IWADs are local-only and must never be committed or recorded
  in a repository checksum file.
- DOOM catalog entries use `:system :deck` and a `:rom` path below
  `/mnt/data/nes-deck/games/doom/` ending in `.wad`. Routing keys off that
  extension, so an owner-maintained local entry needs no code change.
- Never point a DOOM entry's saves into the installed games directory:
  activation replaces it wholesale. Saves belong in `/mnt/data/doom`.
