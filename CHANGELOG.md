# Changelog

All notable changes to Meridian are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While pre-1.0, the scheme is `0.MINOR.PATCH`: `MINOR` for features and
behavioural changes, `PATCH` for fixes. All crates in the workspace share a
single version.

## [Unreleased]

### Fixed

- **Launcher opacity now theme-driven:** the command-palette body was painted
  fully transparent via a hardcoded `LAUNCHER_GLASS_ALPHA = 0`, so `glass_alpha`
  had no effect on it. The body now follows the theme's launcher glass fill, like
  the panel — one `glass_alpha` knob controls launcher transparency.

### Changed

- **Neutral palette nudge:** the default theme colours moved a step further from
  blue toward grey-blue, and `glass_alpha` raised to 0.75 (less transparent).
  These are an interim step; see `docs/GUI_CENTRALIZATION_PLAN.md` for the planned
  central design source + light/dark redesign.

### Docs

- **GUI centralization plan + audit** (`docs/GUI_CENTRALIZATION_PLAN.md`): a
  fresh-context entry point documenting the hardcoded-value audit (file:line),
  the mockup-derived "hint of transparency" targets, the phased plan to route all
  shell/greeter/lock/compositor visuals through one central theme source, and the
  current FreeBSD box runtime state.

## [0.5.1] - 2026-06-19

### Changed

- **Neutral desktop palette:** the shipped `meridian` default theme moves off
  the cool Tokyo-Night blues to a neutral, slightly-cool grey ramp with a single
  blue accent (`#4e99f3`), matching the desktop mockup. Only `[colors]` changed —
  glass, blur, shadow, corner radius, fonts, icons, and cursor are untouched.
- **Desktop wallpaper:** the default theme now ships the same alpine-lake
  wallpaper as the login greeter (`bsd_wallpaper.png`).

## [0.5.0] - 2026-06-19

### Added

- **Adwaita Sans Regular text style:** `meridian-compass-render` now embeds
  Adwaita Sans Regular and exposes it as `TextStyle::SansRegular`, a calmer
  companion to the existing DejaVu Sans Bold for UI labels and display text.
- **Native FreeBSD boot logo (`--quiet-boot`):** `scripts/install-freebsd.sh`
  can install the Meridian loader image to `/boot/images` and drive FreeBSD's
  built-in `boot_mute` framebuffer splash instead of the stock FreeBSD logo. It
  works through `loader.conf` only (no system files patched, so `freebsd-update`
  cannot clobber it), backs up `loader.conf` before the first change, and ships
  `assets/bsd_bootlogo-loader.png`. `docs/FREEBSD.md` documents the flow.

### Changed

- **Login greeter visual refresh:** the greeter palette is now neutral and less
  blue — card fill, input boxes, and the compass guides moved off the cool-blue
  tones toward muted greys. The blurred wallpaper backdrop is precomputed once
  into a cached full-size frame (two-stage bicubic down/upscale) and reused for
  every redraw instead of being convolved per frame. The text caret now sits at
  the text-entry start on empty username, password, and smartcard-PIN fields
  rather than behind the placeholder.

## [0.4.1] - 2026-06-04

### Fixed

- **Compositor glass memory use:** layer-owned glass backdrops now honour
  `glass_blur = false`, matching window titlebars and allowing themes to disable
  live blur consistently across panel, launcher, desktop menu, and popups.
- **Compositor glass blur batching:** multiple glass panes that sample the same
  behind-scene and blur radius now share one rendered/blurred backdrop within a
  frame instead of each rendering the full output into separate offscreen blur
  passes. On the llvmpipe dev VM this reduced the compositor from roughly
  463 MB RSS / 424 MB PSS to roughly 246-258 MB RSS / 212 MB PSS after reboot.

## [0.4.0] - 2026-06-04

### Added

- **Liquid-glass window decorations:** server-side-decorated windows get a
  frosted titlebar. The compositor renders the scene without decorations
  into an offscreen texture, runs a two-pass separable Gaussian blur over
  it, and each titlebar samples that blurred texture behind a tinted,
  rounded pane with a cool-white frame. The window controls fill the
  titlebar height as three colour-tinted glass zones (close/maximize/
  minimize). Theme tokens: `glass`, `glass_blur`, `glass_blur_radius`,
  `glass_tint`, `glass_frame_alpha`, `glass_button_alpha`. `ReloadConfig`
  now forces a full repaint so theme changes apply immediately. A
  blur-behind backdrop for the panel island is present but not yet working
  visually (WIP, see the panel-island commit).

### Fixed

- The window drop shadow is now drawn behind the window content instead of
  in front of it, so opaque clients are no longer dimmed ~40% by the
  shadow overlay; only the soft outer halo remains.


- **Interactive screenshot region picker (portal A2):** the freedesktop
  Screenshot portal's `interactive=true` option now opens a fullscreen
  drag-rectangle picker in the shell instead of capturing the whole
  output. Spectacle-style two-step flow: drag to select, release to
  freeze the rectangle, Enter to confirm, Esc to cancel. Overlay dims
  the rest of the desktop (BGRA alpha 96/255), keeps the selection
  fully transparent so the underlying pixels show through, and draws a
  2-pixel solid white border. The compositor crops the captured
  XRGB8888 framebuffer to the picked region before encoding the PNG —
  the returned `uri` points at a PNG of exactly the selected area.
  Built end-to-end across `meridian-ipc` (interactive flag +
  RegionRequest event + RegionResponse command, with region validation
  flipped from blanket Unsupported to a real nonzero-size check),
  `meridian-compositor` (new `NeedsRegionPick` policy decision routed
  by `metadata.interactive`, a `pending_screenshot_region` queue, and
  per-request region cropping via the existing `crop_xrgb` helper),
  `meridian-shell` (new `region_picker` module with the overlay
  drawing helpers + 4 unit tests, an `Overlay` layer surface anchored
  on all four edges, pointer drag state machine, keyboard
  Enter/Esc handling, and the open/respond methods mirroring the
  consent flow), and `meridian-portal` (reads `interactive` out of the
  Screenshot `a{sv}` options dict and threads it through the bridge
  request metadata). 913 / 913 workspace tests passing. (A2)

- **Access portal backend (auto-allow):** `meridian-portal` now also serves
  `org.freedesktop.impl.portal.Access`. xdg-desktop-portal's Screenshot /
  ScreenCast / Camera / Location front-ends require an Access impl to render
  their own consent dialog before invoking the real backend; without any
  Access impl on the session bus the front-end silently skips those portals
  at startup (the actual failure mode that prevented Screenshot from being
  routed through xdp on the dev VM until this slice landed). Meridian renders
  its own consent UI further down the stack (compositor policy + shell modal
  triggered from `ScreenshotRequestOrigin::PortalDbus`), so the Access impl
  is intentionally a no-op: `AccessDialog` always responds `(0, {})` and lets
  the compositor / shell be the source of truth for consent. With this in
  place xdg-desktop-portal now exposes `org.freedesktop.portal.Screenshot`
  (plus Camera, Location) on `/org/freedesktop/portal/desktop` and routes
  external D-Bus calls through to the new Screenshot backend end-to-end.
  Live verified on the dev VM: portal Screenshot → AccessDialog auto-allow →
  Screenshot backend → bridge to compositor → NeedsConsent → shell modal →
  user answer round-trips back as `(0, {uri})` or `(1, {})`. (A2)

- **xdg-desktop-portal Screenshot backend (portal):** `meridian-portal` now
  serves `org.freedesktop.impl.portal.Screenshot` at the shared portal object
  path. Each `Screenshot()` call opens a short-lived connection to the
  compositor's IPC socket, sends a `ScreenshotBridgeMessage::ScreenshotRequest`
  with `origin=PortalDbus`, demultiplexes broadcast shell events out of the
  reply stream, and returns the compositor's PNG path as a `file://` URI in
  the standard `(response, results{uri})` reply. Maps `PermissionDenied` to
  response code 1 (cancelled) and every other bridge / I/O failure to 2
  (other). `PickColor` is a no-op stub returning 2 to avoid `UnknownMethod`.
  Closes the previously open external route of A2 — capture engine, consent
  state machine, and consent modal are now reachable from real D-Bus callers
  (Flatpak apps, `grim`, GNOME's screenshot tool, etc.). Verified by D-Bus
  introspection of the live service (Screenshot + PickColor methods present,
  Version = 2); end-to-end call goes portal → compositor on the running VM,
  the modal itself lands once the shell-fix from the previous commit is
  installed. (A2)

- **Screenshot consent modal (shell):** when a portal screenshot request needs
  consent, the shell now shows a centered modal ("Bildschirmfoto erlauben? —
  Erlauben/Ablehnen") on an overlay layer that grabs the keyboard. Clicking a
  button (or Enter=allow / Esc=deny) sends `ScreenshotConsentResponse` back to
  the compositor; allow triggers the real capture, deny returns permission
  denied. Completes the interactive-consent path (A2). Verified live end-to-end
  (request → modal → allow → PNG of the desktop written).

- **Screenshot consent state machine (compositor):** portal-routed screenshot
  requests (`origin=PortalDbus`) now resolve to a new `NeedsConsent` policy
  decision instead of an outright deny — the compositor holds the request,
  emits `ShellEvent::ScreenshotConsentRequest{request_id, app_id}` for the shell
  to show a consent modal, and waits. The user's answer arrives as
  `ShellCommand::ScreenshotConsentResponse{request_id, allowed}`: allow moves
  the request to the capture queue, deny replies with permission-denied. The
  consent modal UI itself lands in the next slice; no external caller can reach
  this path yet (the portal interface is still pending). (A2)

- **Screenshot capture engine (compositor):** the screenshot bridge can now
  fulfil a full-output capture — the render loop renders the output, encodes it
  as PNG (XRGB→RGBA, R/B swap) under `$XDG_RUNTIME_DIR`, and replies with the
  file path token. Groundwork for the `org.freedesktop.portal.Screenshot`
  backend (A2). The policy stays **deny-by-default for every origin**; the
  internal capture path is gated behind a dev-only env flag
  (`MERIDIAN_SCREENSHOT_DEV=1`) and is never enabled in a normal session, so a
  self-declared `origin` field can't bypass consent. The external portal route
  and an interactive consent dialog are the next slices. (A2)

- **Settings ▸ Anzeige — scale & rotation:** each output row on the "Anzeige"
  page gains a Skalierung button (cycles 1.0/1.25/1.5/2.0×) and a Drehung button
  (cycles 0/90/180/270°), showing the current value. Both persist per-output to
  `[outputs."<name>"]` (`scale` / `transform`) and live-apply via `ReloadConfig`
  (the compositor reapplies scale + transform on every reload). Config write +
  parse verified live on the dev VM; the same `ReloadConfig` path as the
  existing primary/mode controls. (A4)

- **Settings ▸ Bluetooth — power, scan & pair:** the "Bluetooth" page now shows
  the adapter power state with a toggle, a "Suchen" button that runs a timed
  discovery, and the device list (paired/connected badges; click to pair an
  unknown device or connect a paired one). All `bluetoothctl` mutations run off
  the event loop on a background thread; the read-only snapshot (`show` +
  `devices`) re-polls on entering the page. Power toggle + timed scan verified
  live against a virtual `btvirt` adapter; the pair/connect path is unit-tested
  at the argv/parser level but not exercised against a real peer (no BT
  hardware on the dev VM). (A4)

- **Settings ▸ Network — activate saved connections:** the "Netzwerk" page lists
  saved NetworkManager profiles below the status summary; the active one shows a
  "VERBUNDEN" badge and is inert, the rest are clickable to activate via
  `nmcli connection up id <name>` on a background thread (bringing a link up can
  block for seconds on DHCP/auth — never on the shell event loop). Verified live
  with an Ethernet profile (rc=0). (A4)
- **Settings ▸ Network — Wi-Fi scan & connect:** the "Netzwerk" page now lists
  scanned Wi-Fi networks (SSID, security, signal; the in-use one badged and
  inert). Clicking an open or already-known network connects via
  `nmcli device wifi connect`; a secured unknown network opens an in-page
  password prompt (type + Enter to connect, Esc to cancel — Esc always exits so
  it can't trap input). All nmcli calls run off the event loop on a background
  thread. NOTE: the scan/parse and argv builders are unit-tested and the scan
  query is verified on the dev VM, but the actual connect + password path is
  **not end-to-end tested** — the VM has no Wi-Fi access points. (A4)
- **Settings ▸ Sound — selectable default device:** each output/input row on the
  "Audio" page is now clickable to make that device the default (sink or
  source); the already-default row shows a DEFAULT badge and is inert, others
  get a hover affordance. Drives `wpctl set-default <id>` against the live id
  from the snapshot and re-polls — system state, no config round-trip. (A4)
- **Settings ▸ Cursor — selectable theme:** the "Mauszeiger" page now lists the
  installed cursor themes (scanned from `/usr/share/icons`, `~/.icons`,
  `~/.local/share/icons` — any dir with a `cursors/` subdir) and lets you pick
  one; the choice persists to `[cursor]` and live-applies via `ReloadConfig`.
  Completes the Cursor page (size was already writable). (A4)
- **Settings ▸ Sound — writable volume & mute:** the "Audio" page gains volume
  preset chips (0/25/50/75/100 %) and a mute toggle for the default output;
  these drive `wpctl` against `@DEFAULT_AUDIO_SINK@` and re-poll, so they edit
  live system state directly (no config round-trip). The volume preset is
  clamped to 100 % so a stray id can never amplify past unity. (A4)
- **Settings ▸ Power — writable idle timeout:** the "Energie" page gains a
  chip bar to set the screen-blank idle timeout (Aus / 1 / 5 / 10 / 15 / 30
  min); the choice persists to `[general] idle_timeout_secs` and live-applies
  via `ReloadConfig` (the compositor reads the timeout fresh each render tick,
  so "Aus" disables blanking immediately). (A4)
- **Settings ▸ Cursor — writable size:** the "Mauszeiger" page can now change
  the cursor size (16/24/32/48 px chips); the choice persists to the `[cursor]`
  config section and live-applies via `ReloadConfig`. First writable system
  setting, proving the full write path (widget id → action → config write →
  compositor reload). The cursor theme stays read-only for now. (A4)

## [0.3.0] - 2026-05-29

### Fixed

- Audit M1/M2 — async-signal-safe privilege drop, lock SHM realloc
- Audit L1/L2 -- wipe leaked PAM responses, drop per-frame alloc
- Audit FT-1 -- enforce FreeType face/library drop order
- Audit XW-1 -- clean up X11 windows on non-active workspaces
- Audit GR-1 -- confine move-grab to the window's own workspace
- Audit CFG-1 -- Color::from_str panic on multibyte config values

### Documentation

- Align check commands with the enforced gates

### Tooling

- Drop Codeberg/Forgejo workflows, GitHub-only
- Drop leftover keyboard keylog and gratuitous unsafe Sync

## [0.2.0] - 2026-05-29

### Documentation

- Document versioning and wire up git-cliff

### Tooling

- Auto-publish releases from tags via git-cliff

## [0.1.0] - 2026-05-29

First tagged baseline. Meridian is a Wayland desktop: a Smithay-based
compositor with its own DRM/KMS backend, a separate shell process, a login
manager with boot-splash handover, a session lock, a polkit agent, and a
shared compass renderer.

### Added

- **Compositor** — DRM/KMS backend, tiling window manager, XWayland, window
  decorations as a frosted instrument cluster, gamma-correct UI text, theming.
- **Shell** — floating frosted-glass panel island, launcher, calendar /
  network / audio / workspace popups, system tray, and a desktop context menu
  with a settings flyout (keyboard navigation included).
- **meridian-login** — display manager with boot-splash DRM-master handover;
  password or YubiKey (PIN + touch) authentication.
- **meridian-lock** — session lock screen (`ext-session-lock-v1`), DPMS idle
  blanking, and XDG autostart.
- **meridian-polkit** — authentication agent with setuid-helper PAM flow and
  per-request theme reload.
- **Tooling** — CI on GitHub Actions and Codeberg/Forgejo, a pre-push hook
  running fmt/clippy/test, and unit tests for `meridian-lock` and
  `meridian-polkit`.

[Unreleased]: https://github.com/quompacc/meridian-desktop/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/quompacc/meridian-desktop/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/quompacc/meridian-desktop/compare/v0.4.1...v0.5.0
[0.4.1]: https://github.com/quompacc/meridian-desktop/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/quompacc/meridian-desktop/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/quompacc/meridian-desktop/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/quompacc/meridian-desktop/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/quompacc/meridian-desktop/releases/tag/v0.1.0
