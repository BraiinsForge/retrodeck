# Third-party software and assets

Retro Deck is built on free software. The dashboard's `(c)` screen reads the
authoritative project, role, and SPDX-style license summary from
[`deploy/menu/credits.tsv`](deploy/menu/credits.tsv).

The deployer installs the corresponding upstream notices and license texts at
`/mnt/data/nes-deck/licenses`. Runtime packages carry their own notices, while
the shared static libraries, Nixpkgs source, and CC0 asset
provenance are collected by `nix/runtime-licenses.nix`. The separately pinned
ECL runtime carries ECL, ASDF/UIOP, Boehm GC, libatomic_ops, GMP, and glibc
notices from the exact source archives used in its build.

Source identity is reproducible from these files:

- `flake.lock` pins the emulator cores and Nixpkgs.
- `flake.nix` names every native runtime and its linked libraries.
- `native/vendor/tamalib-rs/UPSTREAM` pins the MIT-licensed Tamagotchi P1
  emulator library used by `tamagotchi-deck`.
- `nix/ecl-arm-static.nix` pins the independent ECL build environment.
- `chiptunes/README.md` and `deploy/menu/ASSETS.md` record the included CC0
  music and settings icons.

The DOOM host combines the pinned fbDOOM fork with the OPL emulator, MIDI
reader, and MUS converter from the pinned Chocolate Doom release. Neither
fbDOOM nor the Deck fork ships a license file, so the GPL-2.0 text their
headers refer to is fetched by hash in `flake.nix` and installed as
`DOOM-COPYING` beside Chocolate Doom's own `COPYING`.

Owner-supplied ROMs are private data. They are not third-party project
dependencies and are not relicensed by this repository. The same applies to
the DOOM IWADs in `deploy/doom/`: only the engine is free software.
