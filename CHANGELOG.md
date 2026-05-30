# Changelog

All notable changes to Meridian are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While pre-1.0, the scheme is `0.MINOR.PATCH`: `MINOR` for features and
behavioural changes, `PATCH` for fixes. All crates in the workspace share a
single version.

## [Unreleased]

### Added

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

[Unreleased]: https://github.com/quompacc/meridian-desktop/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/quompacc/meridian-desktop/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/quompacc/meridian-desktop/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/quompacc/meridian-desktop/releases/tag/v0.1.0
