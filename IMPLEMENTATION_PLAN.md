# Implementation plan

Active work items and their checkpoints. Finished plans get pruned.

## DOOM in the DECK section (2026-07-28)

Integrates `BraiinsForge/forge-doom` as a DECK-section program with controller
support. Decision: follow the `retroHost` pattern rather than run the fork's
binary. The fork drives `/dev/fb0` directly, which cannot work under the BMC
compositor, so `doomLib` compiles the pinned fbDOOM fork into a static archive
with its framebuffer, tty-keyboard, and timer backends replaced by
`native/doom/`, and `doom-deck` links that into a Rust host beside the
emulators. Routing keys off a `.wad` extension on a `:deck` catalog entry, so
further IWADs are catalog-only changes.

- [x] `fbdoom-src` flake input; `doomLib` archive; `doom-deck` host binary
- [x] Video, input, and timer backends; deterministic test-mode clock
- [x] Console-classic controller mapping, always-run, menu rebinding after
      the engine reads its config; keyboard passthrough
- [x] Catalog entry, `:doom` route, exit cross and hold-to-exit supervision
- [x] Deploy and activation: host binary, IWAD staging, `/mnt/data/doom` for
      saves (the installed games directory is replaced on every activation)
- [x] Sound effects: DMX lumps point-resampled and mixed into the host queue
- [x] OPL music: Chocolate Doom's OPL/MIDI/MUS vendored, Retro Deck OPL
      driver replacing the SDL one; `RETRO_DECK_DOOM_MUSIC=0` disables it
- [x] `doom-host-smoke` pinned against Freedoom; `verify-arm-builds` entry;
      policy, catalog, and activation test coverage
- [x] Deployed to the Deck at 10.0.0.15 and measured on screen. Music plays
      and costs nothing measurable: 35.0 fps with no shed frames at the full
      44100 Hz, matching music-off. Two bugs stood in the way. First, Nuked
      OPL3 always emulates at 49716 Hz internally whatever rate it is asked
      for, costing ~80% of a core; lowering its rate from 22050 to 8000
      changed nothing, which exposed it. Replaced with DOSBox dbopl from
      Chocolate Doom 2.2.1. Second, the driver advanced emulated time per
      generated batch rather than per consumed frame, so a caller consuming
      buffered frames advanced no time at all; that stalled the timers the
      chip-detection handshake reads, detection failed, and DOOM dropped the
      music module entirely. Music had never actually played before this fix:
      the "voiced frames" counter had been reporting noise from an
      uninitialised chip. fbDOOM also never calls OPL_InitRegisters, so the
      driver now does that itself.
- [x] Fixed the frame shedding that turned "slightly late" into the reported
      lag spikes: the host shed a present whenever it was late, and because
      the cost was OPL rather than presentation, lateness never recovered and
      43 to 56 of every 60 frames were discarded while the engine still ran at
      35 fps. It now never sheds twice in a row.
- [x] Capped the mixer's catch-up batch. With no open audio device the queue
      reads as permanently empty, so every pass mixed a full target's worth,
      two and a half times what playback consumes.

Open:

- [ ] Owner gameplay test with a controller.

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

Credits crawl (closed 2026-07-27): the "Minecraft lag" was
`dashboard-credits-value` allocating a `(gensym)` per lookup
(~143 us + garbage), six lookups per star per frame in the starfield.
Shared missing-value sentinel + hoisted star loop + deadline-aware
credits poll: render p50 122.6 ms -> 8.2 ms, over-budget frames
690/700 -> 0, verified in the deployed image via `RETRO_DECK_REPL`
probes. The projector fixed-point walk from the earlier attempts
stays; the two symptom-level fixes taught the lesson recorded in the
session memory: measure the production image before theorizing.
