# Implementation plan

Active work items and their checkpoints. Finished plans get pruned.

## GBA support (approved 2026-07-27)

Decision: gpSP libretro core (ARM dynarec — the only realistic 60 fps
core on the dual Cortex-A7), pinned at `5b6e751f4abf368509146cd143c949c1946ac1ae`.
Real `gba_bios.bin` is supplied privately by the owner on the device
(never committed, never uploadable); the core falls back to gpSP's
built-in HLE BIOS with a log line when the file is absent.

- [ ] `gpsp-src` flake input + `gpspCore` static-archive derivation
      (fceumm/gambatte/fuse pattern; dynarec on; GPL-2.0 license file)
- [ ] `gba-deck` retroHost instance; system-directory callback pointing
      at `/mnt/data/roms/gba` for the BIOS; HLE fallback core option
- [ ] 240x160 video path in the host scaler (if not already generic)
- [ ] Catalog: `gba` system in `deploy/menu/games.sexp` (tab, palette,
      ROM dir, covers); regenerate pinned dashboard hashes if geometry
      moves
- [ ] Uploader: accept `.gba`, raise the per-file size cap (GBA ROMs
      reach 32 MB)
- [ ] Smoke: freely-licensed homebrew ROM in `roms/gba/` +
      pinned-frame-hash entry in `libretro-host-smoke` (HLE BIOS in
      the sandbox); `verify-arm-builds` + deploy manifest gain gba-deck
- [ ] Deck test with the owner's BIOS + Slime MoriMori (Japanese dump)
- [ ] Docs: BUILD.md + DECK_NOTES.md (BIOS drop instructions)

## GBC framerate (open)

Mario on GBC judders on the stock 26.07-rc compositor. Measure core
speed headless vs the layer-shell present path; numbers feed
`BMC_COMPLIANCE_PLAN.org` Phase 1 and the upstream protocol asks.
