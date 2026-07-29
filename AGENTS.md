# Repository Instructions

## ROM intake

- `roms/<system>/` is the canonical location for owner-supplied ROMs in this
  private repository. Supported system directories are `nes`, `gb`, `gbc`,
  and `zx`.
- A ROM or single-ROM ZIP placed at the repository root is unprocessed intake.
  Never commit it at the root. Validate the payload, extract it when needed,
  give it a lowercase kebab-case filename, and file it under the matching
  system directory.
- Update `roms/SHA256SUMS`, `deploy/menu/games.sexp`, and the checked-in
  `deploy/menu/games.tsv` whenever a filed ROM is added to the menu.
- Console catalog paths must use `/mnt/data/roms/<system>/<filename>`. Deck
  applications are not ROMs and remain under `/mnt/data/nes-deck/games/`.
- Preserve `.sav`, `.rtc`, `.state`, and emulator configuration sidecars when
  moving a Deck ROM. Keep each sidecar beside its ROM in the same system
  directory.

## Deck program data

- Data files belonging to a Deck program rather than to a console live under
  `deploy/<program>/`, not under `roms/`. The only such directory today is
  `deploy/doom/`, holding the DOOM IWADs.
- Owner-supplied IWADs are private commercial data on the same terms as the
  ROMs above. Record each one in `deploy/doom/SHA256SUMS`.
- DOOM catalog entries use `:system :deck` and a `:rom` path below
  `/mnt/data/nes-deck/games/doom/` ending in `.wad`. Routing keys off that
  extension, so extra IWADs need no code change.
- Never point a DOOM entry's saves into the installed games directory:
  activation replaces it wholesale. Saves belong in `/mnt/data/doom`.
