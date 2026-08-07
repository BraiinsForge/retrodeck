# Tamagotchi P1 firmware

`tamagotchi-deck` uses the owner-supplied Tamagotchi P1 firmware through the
MIT-licensed `tamalib-rs` emulator library. The library does not redistribute
firmware.

Place one compatible 12 KiB P1 dump here as `tama.b`. It is Deck program
data, not a console ROM, and deployment installs it as
`/mnt/data/nes-deck/games/tamagotchi/tama.b` with its original private mode.

The Deck host atomically saves the pet at
`/mnt/data/nes-deck/state/tamagotchi.state`; deployment does not replace that
file. The P1 clock runs at its original real-time rate while the game is open.
Saved sessions deliberately pause while closed, rather than applying an
unbounded offline catch-up.

Record the owner-supplied file in `SHA256SUMS` before committing it:

```sh
sha256sum tama.b > SHA256SUMS
```
