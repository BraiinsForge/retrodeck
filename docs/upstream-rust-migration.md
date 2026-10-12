# RetroDeck: minimum upstream and Rust UI migration

Audit date: 2026-10-12. This is a source audit and implementation plan.
No firmware, application, or UI replacement was deployed during the audit.

## Decision

Target current [BraiinsForge/bmc-main](https://github.com/BraiinsForge/bmc-main)
master and a pure Rust first-party RetroDeck UI. Keep the Common Lisp REPL app.
Preserve the current visuals, interaction, game routing, saves, and application
behavior; reuse the existing Rust mechanisms and external emulators.

The minimum migration is primarily RetroDeck work:

1. Update the Wayland widget protocol and configured-instance identity.
2. Package RetroDeck through the current widget installation path and provision
   its scene through the current BMC configuration/control plane.
3. Replace the Lisp dashboard, settings, credits, Wi-Fi editor, 10 Seconds, and
   chiptune UI with Rust over the existing native library.
4. Replace the Lisp uploader/web UI and catalog compiler, then remove ECL from
   UI binaries and startup. Keep ECL and its terminal route for the REPL.
5. Measure gameplay, pixel sampling, swipes, memory, and resource ownership on
   current master before selecting compositor patches to upstream.

No unconditional compositor/protocol patch is established as necessary by this
source audit. Several fork features have no master equivalent, but current
RetroDeck does not consume them. Hardware parity and foreground resource
ownership are release acceptance gates, not outcomes proved by reading source.

## Compared revisions

The GitHub API and fetched Git ref agreed on the upstream SHA below. The local
GitLab `origin/master` at `dabfe8a7` was stale and is not the comparison target.
The source checkout branches and the existing nearest-filter edit were preserved.

| Repository/ref | Revision | Fork-only commits against current master |
| --- | --- | ---: |
| RetroDeck `master` | `758ec9e` | Application baseline |
| BMC GitHub `master`, fetched as `github/master` | `e0a5c20ceac406ef22f361671516ce5e83e00de8` | Upstream baseline, 2026-10-12 |
| `feature/retro-dashboard-wayland` | `7860917be276bbca31f8fedfd864d4560e517689` | 8 |
| `feature/retro-deck-widget` | `4455b48a3883f8f705e490287b638acbc9cf0de8` | 43 |
| `lho/retrodeck-widget-rate` | `78cc8c7ed0a122f49a742b17ef8c88d7b7dd8c3f` | 1 |

The first two forks share merge base `79aaeb349f5aa4d2d4203eff5e1e754bf299e51c`.
Five rendering/discovery fixes occur as equivalent patches in both forks, so
these counts must not be added as distinct features. `git cherry github/master`
reported every fork commit as `+`: no identical patch is already upstream.
Semantic replacements in newer master still need separate source comparison.

BMC paths below refer to the pinned GitHub revision unless a fork is named.
RetroDeck paths refer to `758ec9e`.

## Mandatory compatibility work in RetroDeck

### Widget protocol and launch identity

RetroDeck currently generates bindings from `protocol/deck-widget-v1.xml` and
binds `deck_widget_manager_v1` in `native/src/wayland.rs:810-840`. Current master
exports `deck_widget_manager_v3` and `deck_widget_surface_v2`. These are different
interface names, not a compatible version increment.

Current upstream contract:

- `get_widget_surface` takes a canonical configured-widget UUID and the surface.
- BMC passes that UUID through `BMC_WIDGET_KEY`, distinct from the widget type UID
  in the manifest. Inherit it into game processes and use it for every surface.
- `params`, `credentials`, and `credential_secrets` arrive as sealed memfd plus
  byte count, rather than JSON strings. Consume or close every descriptor without
  logging credential contents. Wait for the complete configure batch.
- Regenerate bindings from current XML, or reuse the current upstream protocol
  crate. Update event dispatch and lifecycle handling together.

Evidence: [current protocol XML](https://github.com/BraiinsForge/bmc-main/blob/e0a5c20ceac406ef22f361671516ce5e83e00de8/bmc-widget-protocol/protocol/deck-widget.xml),
`bmc-widget-protocol/src/lib.rs:107-137`,
`bmc/src/widget/spawner.rs:48-66`, and
`bmc-openwrt/src/compositor/protocol/dispatch.rs:138-166`.

Keep the current child connection scheme: the registered dashboard opens the
Wayland socket before spawning the game and passes it as fd 9 with
`RETRO_DECK_WIDGET_FD=9`. The compositor therefore sees the registered parent's
peer credentials, not the child's. Keep process-group cleanup and terminal
restoration in `native/src/process.rs`.

The new instance key supplements this connection scheme. When a new client
attaches for the same instance, master forcibly closes the previous client's
Wayland connection, not just its surface. The current parent keeps dispatching
that connection during child supervision (`native/src/process.rs:194-225`).
Adapt launch/return to suspend or detach the dashboard deliberately and reconnect
it after the game exits; account for the compositor's disconnect cleanup and
initial lifecycle replay. Verify repeated dashboard -> game -> dashboard
transitions before accepting the adapter. Source-level replacement support alone
does not establish lifecycle-safe handoff.

Evidence: RetroDeck `native/src/wayland.rs:194-233` and
`native/src/process.rs:341-382,404-416,492-519`; master
`bmc-openwrt/src/compositor/protocol/dispatch.rs:85-92,162-166` and
`bmc-openwrt/src/compositor/protocol/state.rs:228-263,302-329`.

### Package, manifest, and scene installation

The current deployer installs under `/mnt/data/bmc-widgets/retro-deck` and edits
`/etc/bmc_config.json` through `ops/lib/install-bmc-scene.py`. Master now uses
`/etc/bmc/config.json`; the old file is a migration input, not the runtime target
(`bmc/src/startup.rs:1273`, `bmc/src/bin/migrate_config.rs:29-30`).

Build an installable widget tree containing `manifest.json`, `bin/`, and assets,
using the current package/upgrade workflow. Master already packages native
widgets in `nix/widget.nix:21-60` under `lib/bmc-widgets/<name>/`. Validate the
existing RetroDeck manifest against current manifest validation and retain the
1280x480 rectangular viewport on the Deck target.

Provision the fullscreen scene through current scene-management/configuration
facilities. Keep the widget type UID, configured-widget instance UUID, and scene
UUID separate. Replace the legacy file mutation and review activation, update,
removal, and rollback behavior against current BMC packages.

For development with the persistent bundle, master accepts repeated
`--widgets-path` arguments (`bmc-openwrt/src/cli.rs:32-34`). Include both the
profile widget directory and the persistent directory. The fork's automatic
`/mnt/data/bmc-widgets` scan is convenience, not a missing discovery mechanism.
The runtime default is still `/run/current-profile/lib/bmc-widgets`
(`bmc-openwrt/src/main.rs:87-95`), despite older manifest documentation describing
other symlink layouts. Use runtime/package code as the path authority.

Relevant implementation: `bmc/src/widget/registry.rs:184-220`,
`bmc/src/widget/manager.rs`, and `bmc/src/web/grpc/scene_management.rs`.

## Fork feature disposition

These features are concentrated in `feature/retro-deck-widget`. They are useful
building blocks for a different integration architecture, but are not all
prerequisites for the current RetroDeck client.

| Feature and fork commits | Current master alternative or gap | Minimum disposition |
| --- | --- | --- |
| Persistent widget discovery: `34e94c3f` / `d32fff0a` | Profile packages and explicit `--widgets-path` already work; persistent data is not a default scan path. | Use standard packaging or the existing CLI. Upstream the default only if persistent sideloading is the chosen product contract. |
| External native widget/application packaging: `84ff26a2`, `3f1f6c1b`, `3e26be3a`, `615dc57a` | Native widget packaging exists. External-workspace helpers and associated managed-app bundles do not have the same master API. | Package RetroDeck and its current child binaries together. Add only a narrowly needed build integration, rather than the complete fork framework. |
| Managed applications, ownership and process groups: `dc4897c1`, `39e1fe21`, `f07e662c`, `bf4004e3` | No equivalent application registry/supervisor. Master supervises widgets; RetroDeck already supervises its own children. | Reuse `native/src/process.rs`. This feature set becomes required only if BMC must launch/manage individual games. |
| Launch/stop application and system-settings actions: `1897ee20`, `aa082ece` | Master has widget sound/LED action handling and separate system settings interfaces, but not the fork's managed-app requests. | Keep RetroDeck launch/return and settings behavior local. New generic actions are optional interoperability. |
| Renderer tree/state/assets exports and font embedding: `35977ee2`, `c5936066`, `e9d43376`, `ec8b19dc`, `628d0ff6`, `7e168250` | Master has its current renderer/widget APIs and packaged assets, not necessarily the fork's exact exports. RetroDeck already has its own pixel canvas, font, raster and projection mechanisms. | Reuse the RetroDeck renderer for visual parity. These exports are needed only if selecting the fork renderer as the new UI foundation. |
| Wayland keyboard and post-startup input discovery: `d648e791`, `53319488` | Master does not route keyboard input to widgets; its compositor keyboard event path is unused. | Keep RetroDeck's existing evdev keyboard path, with lifecycle/focus checks. Upstream routing only for a deliberate shared-input architecture. |
| Focused gamepad protocol/discovery: `dff76272`, `105b46f5`, `7aa74686`, `a4bd82e6`, `27c0ca8a`, `89ca1aef` | No `deck-gamepad-v1` on master. Current RetroDeck uses its own evdev/gamepad mechanisms. | Keep that implementation for the minimum port. Compositor-scoped gamepad transport is optional. |
| Physical audio serialization and managed PCM: `2d1ba9a3`, `043ab04c`, `7ad44d60`, `9d625402`, `5a0292b1`, `c788b808`, `7ed993b8`, `2484d1fd`, `a5d0e7dd`, `7109deb6` | Master supports system/widget sounds, not this generic foreground PCM application transport. RetroDeck currently uses direct OSS audio. | Keep current audio mechanisms for initial compatibility. Decide and test system-wide audio ownership before release; require a focused upstream service only if local ownership cannot meet that contract. |
| Public scheduler dependency: `6c914c16` | Current master contains its public `bmc-scheduler` implementation. | Use current master dependencies. No scheduler transplant. |

Evidence for master alternatives: `nix/widget.nix`,
`bmc/src/widget/{discovery,registry,spawner}.rs`,
`bmc/src/widget/action_handler.rs:118-332`,
`bmc-openwrt/src/compositor/egl_compositor.rs` keyboard handling,
and the absence of `deck-gamepad-v1`, `deck-audio-v1`, and
`bmc/src/application/{registry,supervisor}.rs` in the pinned tree.
Current RetroDeck consumers are `native/src/{controls,joypad,game_audio,process}.rs`.

## Rendering and performance disposition

| Fork change | Current master alternative or gap | Upstream decision |
| --- | --- | --- |
| Teardown diagnostics: `a6f45811` / `f2e003a7` | Master has host teardown/error paths; the exact extra warning context is absent. | Observability improvement, not a migration dependency. |
| DMA-BUF descriptor recovery/cache ownership: `2b5e710d`, `88657444` / `3806cd2d`, `76ac088e` | Master still caches and duplicates DMA-BUF fds in `bmc-widget/src/egl.rs:778-797`; the fork recovery/ownership changes are absent. | Candidate correctness fixes with a reproducible descriptor failure. RetroDeck sends SHM, so these are not direct client API requirements. Stock widgets may still benefit. |
| Prepared buffers: `445c3f9d` / `42dba1f3` | Master has newer prepared/two-buffer lifecycle handling in `bmc-wasm-host/src/render_target.rs:57-65,312-345,416-431`. | Use current lifecycle handling and test swipe parity before porting the older replacement. |
| Idle scene release and neighbor preparation: `8d770442`, `52c04beb`, `4455b48a` | Master has dormant/prepared lifecycle and later readiness fixes, including `5bae7460a` and `7a7970666`. These are semantic alternatives, not identical patches. | Measure RAM and scene readiness. Select a current-master fix only for a demonstrated gap. |
| Reuse SHM textures: `5a23aed6` | Master reimports widget SHM textures through `ShmImport::Always` in `scene_renderer.rs:640-707`; the reuse optimization is absent. | Performance candidate if texture import/allocation limits measured gameplay. Preserve repaint/damage correctness. |
| Defer GPU lock release to page flip: `b77f5a1c` | Master retains explicit fence/finish completion waits and has newer GPU/render-lock work. | Separate synchronization/performance review, driven by current-master traces. |
| Direct gameplay scanout: `7860917b` | Master retains composited output and optional `ScanoutSwizzler`, now also used for platform color adjustment. | Separate optimization. Validate format, rotation, capture, color adjustment and damage before selecting it. |
| Remove frame-callback cap: `78cc8c7e` | Master still has the 32 ms per-widget callback cap (`state.rs:87-100,680-690`). RetroDeck never requests `wl_surface.frame`. | No minimum change for the existing self-paced client. Revisit if choosing a callback-paced client that needs native game rates. |
| Global nearest-neighbor patch: `patches/bmc-nearest-neighbor-filter.patch` | No compositor per-widget filter selector was found. Integer scaling in the client already exists; current-master pixel parity is untested. | Check actual sampling on Deck. If a change is needed, propose a scoped pixel-art filter choice rather than making every system widget nearest-neighbor. |

The initial upstream queue therefore contains no compulsory bulk cherry-pick.
The first candidates to investigate are pixel-art sampling, SHM texture reuse,
then measured GPU/scanout bottlenecks. Descriptor correctness is a separate
small bugfix series when its failure is reproduced.

## Implementation progress

10 Seconds now uses a standalone Rust executable built without ECL.
`native/src/ten_seconds.rs` owns timing, input priority, cues and pixel rendering.
Deployment installs the binary and removes the old Lisp timer source.
Five Rust tests pass, including all five reference RGB565 frame hashes; the
static ARM binary passes its QEMU usage check. Physical Deck acceptance is pending.

Catalog generation now uses `compile-catalog`, a Rust executable without ECL.
The launcher calls it directly and retains the generated-cache and fallback TSV
selection. The Lisp compiler is deleted. Twelve Rust tests cover bounded parsing,
schema/palette validation and output handling; generated TSV bytes match the
checked-in files on the host and under ARM QEMU.

Chiptunes now use `chiptune-deck`, a standalone Rust GME/Vorbis player.
The Lisp player, scanner/display policy, entrypoint and compilation are deleted.
Five Rust tests cover scanning, input, playback transitions and muted pacing;
both reference frame hashes match. The static ARM entrypoint passes QEMU.
Physical input and audio acceptance is pending.

## Rust-only UI inventory

This inventory records the audited `48f66a1` baseline, before the replacements above.
`ops/deploy.sh:120-123` links `deck-menu`, `ten-seconds-deck`, and the shared host;
`native/src/main.rs:1471-1472` calls `RETRODECK:MAIN`.
`lisp/startup.lisp:773-803` dispatches dashboard, 10 Seconds, and chiptunes and
loads their implementation files. The ARM build also supports the compiled
`lisp-image` feature (`flake.nix:554-565`).

| Current source | Rust replacement scope |
| --- | --- |
| `lisp/ui.lisp`, `lisp/dashboard.lisp` | Layout, covers, tabs/carousel, rendering, reducer/runtime loop, status and launch/return. |
| `lisp/policy.lisp`, `lisp/process.lisp` | Routing, input/modal priority, timers, cue/ownership policy and child recovery. Call existing Rust process mechanisms directly. |
| `lisp/settings.lisp`, `lisp/wifi.lisp`, `lisp/credits.lisp` | Settings persistence/effects, exact Wi-Fi editor/helper contract, credits rendering and interaction. |
| `lisp/timer.lisp`, `lisp/chiptune.lisp` | 10 Seconds and the chiptune player UI/runtime. Keep existing Rust audio/decoder mechanisms. |
| `lisp/startup.lisp`, `native/src/main.rs`, `nix/lisp-image.nix` | Replace the ECL-driven UI bootstrap/FFI with normal Rust entrypoints and direct library calls. |
| `deploy/menu/compile-catalog.lisp` and launcher ECL invocation | Implement catalog/palette validation and compilation in Rust with the existing input/output contracts. |
| `lisp/uploader.lisp`, `deploy/uploader/run.lisp` | Replace the first-party uploader HTTP service and HTML UI with Rust. Preserve upload/authentication/catalog activation behavior. |

At that baseline, the nine UI/policy implementation files contain 5,807 lines;
startup adds 803, the uploader 563 and the catalog compiler 414.
not a Rust line-count estimate. The 1,649-line native entrypoint is largely the
ECL/primitive boundary, not an existing Rust dashboard to enable.

Reuse `native/src/lib.rs` modules for Wayland, canvas/font/raster/projection,
input/gamepads, audio/chiptunes, process control, settings files, network and
framebuffer fallback. Keep `retro-host`, `doom-host`, `tamagotchi-host`, external
emulator cores, fbterm and mature decoder/runtime dependencies.

Keep catalog S-expressions as data if useful. Rust can parse the existing schema
without evaluating Lisp. Preserve generated TSV/palette/credits formats and
fallback behavior during the port. Port the compiler/uploader when removing
runtime Lisp outside the REPL, rather than changing file formats and UI at once.

The separate REPL route is `deploy/terminal/retro-terminal` in `lisp` mode with
ECL, its persistent `/mnt/data/langs/lisp` directory and history. Its interpreter
and user programs are independent of the UI Lisp tree. Keep that app when removing
UI source installation and ECL linkage from dashboard/game/player binaries.

## Smallest implementation sequence

1. **Current-master bridge.** Update protocol bindings, `BMC_WIDGET_KEY`, memfd
   payload handling and child identity propagation in the shared Rust library.
   Exercise a small Rust widget and game launch/return against current master.
   Update all widget-presented child hosts together. This establishes protocol
   compatibility independently of rewriting dashboard policy.
2. **Installable package.** Produce the native widget package, validate the
   manifest, and replace legacy scene/config provisioning. Include current game
   hosts and REPL runtimes. Prove install/update/removal without requiring the
   fork's application supervisor.
3. **Rust dashboard.** Add a Rust `deck-menu` over the existing native library.
   Port the reducer/rendering, settings, Wi-Fi, credits, input policy, audio cues
   and launch/return as one usable replacement. Use existing Lisp behavior and
   fixtures as the differential reference while developing it.
4. **Rust app UI and supporting services.** Port 10 Seconds and the chiptune UI;
   port the catalog compiler and uploader in their own focused changes. Preserve
   catalog outputs, upload responses, authentication and state/save paths.
5. **Remove the UI Lisp bootstrap.** Switch production entrypoints after parity,
   remove UI `lisp-image`/ECL linkage and installed UI Lisp sources, and simplify
   build/deploy/test plumbing. Keep the ECL REPL package and launcher.
6. **Evidence-backed upstream series.** Run the acceptance matrix on unmodified
   current master. Upstream only gaps demonstrated there, as independent changes
   rebased onto master, rather than merging either July fork wholesale.

Steps 1-2 establish the path to current master. Steps 3-5 satisfy the Rust-only UI
release target. Package/protocol work can proceed without choosing the fork's
renderer, managed applications, gamepad transport or managed PCM architecture.

## Verification and release decisions

- Host differential frames/state transitions for dashboard, modals, credits,
  timer and player; catalog accepted/rejected inputs and exact generated bytes.
- Current protocol integration: configured UUID, configure/memfd events,
  lifecycle/disconnect, repeated dashboard/game/dashboard handoff, child crashes,
  shutdown, and scene swiping. Include the inherited parent socket path.
- ARM build and QEMU startup for Rust UI and each host; separate ECL REPL launch.
- Package install, update, scene creation, uninstall and rollback using current
  BMC paths/APIs, preserving saves and user programs.
- Physical Deck frames, pixel edges, keyboard/THEGamepad hotplug and touch,
  audible cues, input during sound, terminal/REPL return, native game pacing,
  RAM/swap and coexistence with normal system widgets.
- Explicit audio/input focus contract: verify background scenes do not react to
  raw input or retain unintended audio ownership, and alarms/system sounds have
  the agreed behavior during games. This may justify generic upstream ownership
  mechanisms even though the present client can run without the fork protocols.

Hardware acceptance has not been performed against `e0a5c20c`. The source audit
separates mandatory client incompatibilities from optional features and measured
performance work; it does not certify release readiness.
