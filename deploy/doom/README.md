# DOOM game data

`doom.wad` is the IWAD the DECK section's **DOOM** entry loads. The deployer
copies every `*.wad` in this directory to
`/mnt/data/nes-deck/games/doom/` and the catalog points at it there.

## Ownership and licensing

The tracked `doom.wad` is owner-supplied commercial game data, like the
console ROMs under `roms/`. It is private to this repository, is not
redistributable, and is not relicensed by this project. Only the fbDOOM
engine is free software; see [THIRD_PARTY.md](../../THIRD_PARTY.md).

Verify the tracked copy:

```sh
cd deploy/doom && sha256sum -c SHA256SUMS
```

## Adding another IWAD

Drop it here and add a catalog entry in `deploy/menu/games.sexp` with
`:system :deck` and a `:rom` path below `/mnt/data/nes-deck/games/doom/`
ending in `.wad`. Routing is by extension, so no code changes are needed:

```lisp
(:id "doom2"
 :title "DOOM II"
 :system :deck
 :rom "/mnt/data/nes-deck/games/doom/doom2.wad"
 :color "#D75F5F")
```

Regenerate `deploy/menu/games.tsv` afterwards, as
[deploy/menu/README.md](../menu/README.md) describes.

## Saves

DOOM writes its `savegame/` directory below `/mnt/data/doom`, **not** beside
the WAD. The installed games directory is replaced wholesale on every
activation, so saves stored there would be deleted by the next deployment.

DOOM's own settings do not persist between launches: fbDOOM compiles out the
body of `SaveDefaultCollection`, so no `default.cfg` is ever written even
though the engine announces one. This costs nothing in practice, because the
host applies the controller mapping and always-run after the engine reads its
configuration, and volume comes from the dashboard.

Sound effects and OPL music are both on by default and hold DOOM's full 35 Hz
tic rate on the Deck.

The sandboxed frame-hash test uses Freedoom instead of this file, pinned in
`flake.nix`, because Freedoom is redistributable.
