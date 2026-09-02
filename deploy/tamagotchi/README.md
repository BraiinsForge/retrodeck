# Tamagotchi P1 firmware

`tamagotchi-deck` uses an owner-supplied compatible 12 KiB P1 firmware dump
named `tama.b` through the MIT-licensed `tamalib-rs` emulator library. Firmware
is deliberately untracked.

Deployment installs a local firmware file at
`/mnt/data/nes-deck/games/tamagotchi/tama.b` with its original private mode.
The Deck host saves state atomically at
`/mnt/data/nes-deck/state/tamagotchi.state` and pauses while the game is closed.
