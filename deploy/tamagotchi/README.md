# Tamagotchi P1 firmware

`tamagotchi-deck` uses the owner-supplied Tamagotchi P1 firmware through the
MIT-licensed `tamalib-rs` emulator library. The library does not redistribute
firmware.

Place one compatible 12 KiB P1 dump here as `tama.b`. It is Deck program
data, not a console ROM, and deployment installs it as
`/mnt/data/nes-deck/games/tamagotchi/tama.b` with its original private mode.

Record the owner-supplied file in `SHA256SUMS` before committing it:

```sh
sha256sum tama.b > SHA256SUMS
```
