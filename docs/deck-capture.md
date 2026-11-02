# Deck screen capture

## Installed framebuffer

On the inspected legacy Deck, `/dev/fb0` is `stmdrmfb`: 600×1280 active pixels,
16-bit little-endian RGB565 and a 1,280-byte row stride. Each stored row includes
40 padding pixels. Read the framebuffer over SSH, crop the padding, then rotate
clockwise for the 1280×600 dashboard. Run from the repository root with host
FFmpeg installed:

```sh
target=root@DECK-IP
mkdir -p result-device-capture
ssh -o BatchMode=yes -o ConnectTimeout=8 -o LogLevel=ERROR "$target" \
  'dd if=/dev/fb0 bs=1280 count=1280 2>/dev/null' \
  > result-device-capture/deck-framebuffer.rgb565
test "$(wc -c < result-device-capture/deck-framebuffer.rgb565)" -eq 1638400 &&
ffmpeg -hide_banner -loglevel error -y \
  -f rawvideo -pixel_format rgb565le -video_size 640x1280 \
  -i result-device-capture/deck-framebuffer.rgb565 \
  -frames:v 1 -vf 'crop=600:1280:0:0,transpose=1' -update 1 \
  result-device-capture/deck-current.png
```

Verify framebuffer format and stride before using these constants on another
firmware. The size check detects truncated reads. This method reads device
memory without installing a capture utility or restarting a service.

The 2026-11-02 capture produced an upright 1280×600 PNG showing the installed
console tabs, carousel, covers and settings icon. The dashboard and compositor
retained their PIDs. Captures belong in the ignored `result-device-capture/`
directory because they can contain owner-supplied artwork.

A framebuffer snapshot is not a panel photograph or a synchronized scanout
capture. It does not establish physical pixel-edge fidelity, animation pacing,
or current-master Wayland integration.

## Current BMC master

At `5ce80092ed165bb1b75bbb54d2e672ad8cde00df`, BMC already implements
`ext-image-copy-capture-v1`. Capture defaults to enabled in
`bmc-openwrt/src/compositor/state.rs:420`; the existing SHM capture client is
`bmc-virt/relay/src/capture.rs`. Use that protocol/client when testing a compatible
compositor rather than adding a compositor screenshot hook.

The framebuffer procedure above was tested against the installed legacy image.
The current-master Wayland capture path was source-inspected, not exercised on
that device. Preserve capture behavior when evaluating direct scanout or other
compositor optimizations.
