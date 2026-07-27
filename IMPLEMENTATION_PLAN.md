# Implementation plan

Active work items and their checkpoints. Finished plans get pruned.

## GBA support (approved 2026-07-27)

Decision: gpSP libretro core (ARM dynarec — the only realistic 60 fps
core on the dual Cortex-A7), pinned at `5b6e751f4abf368509146cd143c949c1946ac1ae`.
Real `gba_bios.bin` is supplied privately by the owner on the device
(never committed, never uploadable); the core falls back to gpSP's
built-in HLE BIOS with a log line when the file is absent.

- [x] `gpsp-src` flake input + `gpspCore` static-archive derivation
      (fceumm/gambatte/fuse pattern; dynarec on; GPL-2.0 license file)
- [x] `gba-deck` retroHost instance; system-directory callback pointing
      at `/mnt/data/roms/gba` for the BIOS; HLE fallback core option
- [x] 240x160 video path in the host scaler (if not already generic)
- [x] Catalog: `gba` system in `deploy/menu/games.sexp` (tab, palette,
      ROM dir, covers); regenerate pinned dashboard hashes if geometry
      moves
- [x] Uploader: accept `.gba`, raise the per-file size cap (GBA ROMs
      reach 32 MB)
- [x] Smoke: freely-licensed homebrew ROM in `roms/gba/` +
      pinned-frame-hash entry in `libretro-host-smoke` (HLE BIOS in
      the sandbox); `verify-arm-builds` + deploy manifest gain gba-deck
- [x] Deck test with the owner's BIOS + Slime MoriMori (Japanese dump)
- [x] Docs: BUILD.md + DECK_NOTES.md (BIOS drop instructions)

## Gameplay performance (diagnosed 2026-07-27)

Instrumented on the layer-shell path (`RETRO_DECK_RUNTIME_DIAGNOSTICS`):
GBC and GBA hold exactly 60 fps client-side; NES saturated the machine
(nes-deck 44% + compositor 18% per core, 14% idle) and fell ~10% behind
in heavy scenes, draining the audio queue to zero — the audible chop.
Fixed client-side: presents are shed only when the frame pacer is
already half a frame late, and every game starts with a ~30 ms silent
audio cushion. Post-fix NES holds 0.99-1.01 s per 60 frames.

Remaining, inherently: the 63 Hz panel duplicates ~3 frames/s of
59.7 Hz content (micro-stutter in smooth scrolling), and the
compositor repaints occluded widgets under the game on every commit
(~18% CPU) — both are `BMC_COMPLIANCE_PLAN.org` Phase 2 upstream asks.
