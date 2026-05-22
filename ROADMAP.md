# Meridian — Roadmap (forward-looking)

> Companion to `PLAN.md` (which is the historical initial-buildup plan).
> This file is the working "what's next" view, organised by daily-driver
> readiness milestones.

Last updated: 2026-05-22 (after Phase 8 IPC handover, security audit,
and post-audit hardening landed).

## Where we are now

The full boot chain runs end-to-end on the linux-dev VM:

```
bootsplash  →  meridian-login  →  meridian (compositor)  →  meridian-shell
   spin           Card+PAM             wallpaper                panel+launcher
```

DRM/KMS backend with libseat, xdg-shell + layer-shell + xwayland +
screencopy + session_lock + dmabuf + idle stack are in. The login path
has been security-reviewed and hardened (IPC socket chown'd to authd
user, password zeroized on drop).

What follows is the path from "author's experimental desktop" to "real
people can use it".

---

## Phase A — Self-Daily-Driver (next ~4-6 weeks)

> Author runs Meridian on a dedicated machine, accepting rough edges.

| # | Item | Effort | Why |
|---|------|--------|-----|
| A1 | **Notification daemon** — `org.freedesktop.Notifications` dbus interface + popup rendering in `meridian-shell` (reuse the `context_menu` engine for the popup surface) | 1-2 weeks | Browsers, messengers, build tools all expect this. Without it, notifications silently disappear. |
| A2 | **xdg-desktop-portal v1** — file picker, screenshot, screen-share via `meridian-portal` | 2-3 weeks | Flatpaks, browser screen-share, "open file" dialogs all route through portals. |
| A3 | **Settings UI v1** — display + keyboard + audio + theme | 1-2 weeks | Without it every adjustment is a TOML edit + restart. |
| A4 | **Multi-monitor hotplug stable** (README flagged in-progress) | ongoing | First thing that breaks when you plug into a beamer or dock. |

## Phase B — Tech-User-Daily-Driver (~3 months after Phase A)

> A Linux-savvy friend can install and use Meridian on their own gear.

| # | Item | Effort |
|---|------|--------|
| B1 | System tray (StatusNotifierItem dbus) — needed by Slack/Discord/Element/etc. | 2-3 weeks |
| B2 | Panel applets: audio (PipeWire), network (partial already in shell/network/), bluetooth, battery, brightness | 1-2 weeks each |
| B3 | Fractional scaling — for HiDPI laptop + FHD external setups | 2-3 weeks |
| B4 | Lock screen UI + idle timer — `session_lock.rs` exists, the front-end doesn't | 1-2 weeks |
| B5 | Input methods — `text_input_v3` + IBus/fcitx bridge for CJK | 2-3 weeks |
| B6 | Clipboard manager + cross-app drag-and-drop polish | 1-2 weeks |

## Phase C — Real-Daily-Driver (~6-12 months after Phase B)

> Someone who isn't willing to debug their desktop can rely on it.

| # | Item | Effort |
|---|------|--------|
| C1 | Stability hardening — memory leak hunt, crash recovery, long-session burn-in | continuous |
| C2 | Compatibility matrix — Firefox / Chromium / Steam / Electron / LibreOffice / GIMP, edge cases | continuous |
| C3 | Color management + night mode | 2-3 weeks |
| C4 | Power management — brightness keys, suspend/resume, lid-close behavior | 2-3 weeks |
| C5 | User-facing docs (not dev-facing) | 1-2 weeks |
| C6 | Distribution — Debian package / AUR / Flatpak manifest | 1-2 weeks |
| C7 | Update mechanism (or deliberate hand-off to distro PM) | 1 week |

## Explicitly out of scope (for now)

- VR / AR, exotic displays
- Custom auth stack — PAM stays
- Custom sound server — PipeWire stays
- Mobile / touch-first UX — that's Phosh territory
- Gaming-specific (gamescope integration, controller support) — Phase C+ if at all

## How to use this document

Treat each item as a small project. Before starting one:
1. Read the existing code surface it touches (e.g. for A1, look at
   `crates/meridian-shell/src/context_menu.rs` for the popup engine).
2. Sketch the protocol surface in a short design note.
3. Implement on a feature branch, land in small commits.
4. Update this file's status when shipping (strike-through completed
   items rather than deleting — keeps the trail visible).
