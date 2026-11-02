# Native widget compatibility

Source target: [BraiinsForge/bmc-main master, 2026-11-02](https://github.com/BraiinsForge/bmc-main/tree/b2c945b5f7a8ea3a7d38a8410a1ca64e8b69b9af),
`b2c945b5f7a8ea3a7d38a8410a1ca64e8b69b9af`.

RetroDeck is a native ARM widget. Its first-party dashboard, uploader, catalog
compiler, timer and chiptune UI are Rust executables. ECL Common Lisp, Chibi
Scheme, Lua and MicroPython are separate language applications.

## Required BMC additions

None for native discovery, process spawning, keyed surface replacement, package
installation or scene registration at the source target. Use the stock widget
package/profile and version-2 configuration contracts below. Hardware acceptance
is separate from this source compatibility assessment.

| Capability | Stock BMC contract | RetroDeck implementation |
| --- | --- | --- |
| Native execution | Execute the manifest's `binary` directly | Static ARM dashboard and catalog compiler |
| Discovery | `/run/current-profile/lib/bmc-widgets` | `.#retro-deck-widget` package and stock profile merge |
| Instance identity | `BMC_WIDGET_KEY` configured-widget UUID | Dashboard and game children use the same key |
| Surface handoff | Same-key attachment closes the old client's connection | Suspend dashboard dispatch during a game; reconnect on return |
| Supervision | Restart on supervised process exit | Keep dashboard alive during games; supervise child groups locally |
| Configuration | Persisted `/etc/bmc/config.json`, version 2 | Validate and provision one fullscreen scene atomically |

Evidence at the pinned revision:
`bmc/src/widget/{spawner,manager,discovery}.rs`,
`bmc-openwrt/src/compositor/protocol/{dispatch,state}.rs`,
`bmc-openwrt/src/compositor/egl_compositor.rs`,
`bmc-nix/src/bin/cli.rs`, and `bmc/src/config.rs`.

## Package and installation

The package contains the manifest, launcher, static dashboard/compiler, credits,
palette, settings icon and runtime license notices under
`lib/bmc-widgets/retro-deck`. It has no runtime Nix store references. The wrapper
resolves its immutable package through profile symlinks and passes that root to
the packaged launcher.

Deploy application runtimes and persistent content separately under
`/mnt/data/nes-deck`. Owner catalogs, uploads, covers, settings, saves and REPL
history use the persistent paths. Console media belongs under
`/mnt/data/roms/<system>`; DOOM saves belong under `/mnt/data/doom`.
Owner media is excluded from the widget package.

Before stopping services, deployment validates:

- The mounted, initialized Nix store and active `/run/current-profile`.
- Stock `/etc/init.d/bmc-compositor`, `nix-store`, `bmc-nix-cli` and `readlink`.
- A persisted version-2 `/etc/bmc/config.json`. Complete BMC's configuration
  migration first if the on-disk version is older.

Deployment imports the NAR, checks the imported ARM executable, merges it with
`bmc-nix-cli add-packages --base current`, activates the profile, and verifies the
resolved active package. Only then does it register the scene and restart BMC.
This final restart accounts for activation hooks that start BMC before the new
scene is written. Existing scenes, settings, UUIDs and disabled instances are
preserved; the original configuration is backed up once.

The manifest type UUID, configured widget UUID and scene UUID are distinct.
BMC exports the configured instance UUID as `BMC_WIDGET_KEY`. A rejected or
inactive key is not an invitation to invent a new instance.

Use explicit `--presentation framebuffer` only on a Deck without the compositor
service. See [BUILD.md](../BUILD.md#stock-bmc-installation) for deployment commands.

## Protocol, lifecycle and process ownership

Bindings are generated from the unmodified upstream XML; see
[protocol provenance](../protocol/PROVENANCE.md). Negotiate manager/surface
version 2, with version 1 compatibility. Optional language events use the
protocol's English fallback. RetroDeck declares no params or credentials and
closes their payload FDs, including runtime updates.

`configure_done` completes geometry/settings configuration. Initial lifecycle
arrives separately. Do not draw before both configuration and a renderable
lifecycle. Prepared permits one static frame; Entering, Visible and Leaving
permit continuous rendering. Dormant and unknown states do not render.

Game clients connect through ordinary `WAYLAND_DISPLAY`/`XDG_RUNTIME_DIR`
discovery. There is no FD-9 transfer or extra native registration handshake.
During game ownership, the dashboard does not dispatch its replaced connection.
After the child exits or crashes, it discards that connection and reopens the
configured instance. Terminal mirroring uses the dashboard's surface.

The stock manager supervises the dashboard process, not its entire process tree.
RetroDeck applies its normal TERM/KILL grace to child groups and installs a
race-checked Linux parent-death signal for directly launched executables.
Arbitrary descendants and credential-changing execs need separate process-tree
supervision if introduced.

Console and DOOM loops pause in Prepared/Dormant, discard queued game audio and
reset the return hold. They resume with active lifecycle and terminate on
shutdown or compositor loss. Framebuffer and headless execution use their normal
pacing. Touch comes from Wayland in widget mode; the direct Goodix grab belongs
to framebuffer mode. USB controllers and keyboard fallback use local input.

## Optional upstream work

| Addition | When it is justified |
| --- | --- |
| PCM mixing/audio focus | Required if native `/dev/dsp` cannot coexist with alarms and system sounds under the agreed policy; stock named sounds are not a PCM stream API |
| Mediated keyboard/gamepad focus | Required if local input cannot meet foreground ownership and hotplug requirements; stock compositor forwards touch, not keyboard/gamepad input |
| Scoped nearest-neighbor filtering | Only if physical pixel-edge comparisons fail; the global local patch is an experiment, not an installation dependency |
| Process-tree/cgroup supervision | Useful for generic descendant cleanup beyond RetroDeck's directly supervised native children |
| SHM import reuse or direct scanout | Only after profiling; preserve damage, rotation, color adjustment and capture correctness |

Fork-only persistent discovery, application registries, rendering exports and
custom gamepad/audio protocols are not prerequisites for the current package.
Keep any demonstrated upstream fix small and independent rather than merging a
historical RetroDeck fork wholesale.

## Verification and physical acceptance

Automated coverage includes full-frame UI references, catalog validation and
exact output, uploader HTTP/authentication, configuration/profile transactions,
canonical keys, payload FD closure, full SHM frame contents/bounds/releases,
v1/v2 negotiation, delayed lifecycle, Prepared/Dormant pause/resume,
shutdown/disconnect, repeated same-key replacement/reconnection, rejected keys,
and dashboard-death child termination. ARM/QEMU checks cover the native package,
application hosts and separate language runtimes.

```sh
cargo clippy --manifest-path native/Cargo.toml --locked --all-targets --no-deps -- -D warnings
cargo test --manifest-path native/Cargo.toml --locked --all-targets
tests/run-host-tests.sh
tests/verify-arm-builds.sh
nix build --no-link .#checks.x86_64-linux.libretro-host-smoke
```

The local Wayland fixture is not the BMC manager/coordinator. On 2026-11-02,
7 actual stock socket-pair tests in `compositor::state::compositor_protocol_test`
and 23 state tests in `compositor::protocol::state::tests` passed. They cover
admission, language, replacement, retained registration and stale attachments.
They ran from a private archive of the pinned revision using its `.#local` dev
shell and Git LFS assets. This is not a supervised RetroDeck/stock-compositor
integration run.

No device deployment or physical acceptance was performed for this revision.
Before release, test repeated dashboard/game/dashboard handoff and scene
activation/deactivation on stock BMC, including compositor restart, failed exec,
child crash, install/update/uninstall and rollback. Check pixel edges, pacing,
RAM/swap, swipes, touch, keyboard/THEGamepad hotplug, audible cues and terminal/REPL
return. Confirm that off-screen apps ignore input and that `/dev/dsp` access and
alarm/system-sound contention meet the agreed audio policy.
