# Plan: Uniform Meridian window frame (SSD) matching the mockups

**Goal:** every app window shows Meridian's own titlebar + window controls,
styled to [`assets/bsd_desktop_mockup.png`](../assets/bsd_desktop_mockup.png)
(dark) and [`assets/bsd_desktop_mockup_hell.png`](../assets/bsd_desktop_mockup_hell.png)
(light) — the Wayland-correct equivalent of Cinnamon's uniform X11 WM titlebars.
Everything from `meridian-tokens` (design_guard must stay green).

---

## Why this is the only clean path (findings from the 2026-06-21 session)

These were established empirically on the Arch box — **do not re-litigate**:

1. **libadwaita GTK4 apps cannot be restyled by CSS** — only *recoloured*. A
   stark "red test" (`headerbar { background:#f00 !important }` in
   `~/.config/gtk-4.0/gtk.css`) left the headerbar dark. libadwaita honours only
   the named `@define-color` overrides, not widget/window-control CSS.
2. **Cinnamon looks uniform because it is X11** — the WM (Muffin) draws *every*
   titlebar. On Wayland, apps self-decorate (CSD). So "like Cinnamon" on Wayland
   = the compositor draws the frame (SSD).
3. **Meridian's SSD frame is the lever** — `crates/meridian-compositor/src/decoration/`
   draws the titlebar + buttons in Rust, token-driven, full control. Nemo's
   "Home" titlebar is already Meridian SSD (that's why GTK CSS didn't touch it).
4. **`GTK_CSD=0` makes GTK3 apps use SSD** — verified: Xviewer got the Meridian
   frame with `GTK_CSD=0`. libadwaita ignores `GTK_CSD` (test forced-SSD instead).
5. **Naive force-SSD double-framed Chromium** (Meridian SSD + Chromium's own CSD).
   The fix is force-SSD **plus** suppressing the app's own CSD controls.
6. **Current decoration policy** (after commit C9): `request_mode` *honours* the
   client mode; `new_decoration`/`unset_mode` default ServerSide
   (`state/handlers/misc.rs`). `state/handlers/xdg/lifecycle.rs:47` sets
   `set_ssd(false)` on new toplevels initially.
7. **App-launch quirk:** GTK apps need `GDK_BACKEND=wayland` or they
   intermittently fail with "failed to create display".

---

## Steps

### Step 1 — Style Meridian's SSD decoration to the mockup
Files: `crates/meridian-compositor/src/decoration/{model.rs, mod.rs, render/elements.rs, render/*}`; values from `meridian-tokens`/`meridian-config` `Decorations`.
- Titlebar tone (surface/bg), height, rounded top corners, title text placement.
- Window buttons —□×: **flat, minimal, light-grey symbols** (see the mockup
  terminal titlebar — no filled circles), subtle hover bg; close is grey in the
  mockup (not red) — confirm with the user.
- No hard-coded colours/alpha/radius (design_guard); add to tokens if missing.
- **Verify:** screenshot an SSD app (a terminal / XWayland app) vs the mockup.

### Step 2 — Force apps onto the SSD frame + suppress their CSD
- Compositor: force `ServerSide` in `request_mode` again (re-apply the reverted
  C6 change) — but now the SSD frame is styled, so it's wanted.
- GTK apps: export `GTK_CSD=0` in the session env (set in the compositor process
  before it spawns children, and/or in the shell's `activate_user_session`
  import list) so GTK3 apps drop CSD and use the Meridian frame. Consider
  `gtk-decoration-layout` minimal/empty.
- **libadwaita: TEST** whether forced ServerSide makes GNOME apps drop their CSD
  controls (GNOME apps *can* accept server decorations). If yes → uniform. If
  they double-frame → Step 3.
- Chromium: its "Use system title bar and borders" preference (pre-seed the
  profile `Preferences`: `"browser":{"custom_chrome_frame":false}`) or accept CSD.

### Step 3 — Reconcile the app stack with the frame reality
- If libadwaita can't be forced to SSD cleanly, the Cinnamon-true choice is to
  prefer **GTK3 X-Apps** for the defaults (Xviewer/Xed/Nemo + a GTK3 terminal),
  where SSD + theming are fully controllable. Trade-off: less "modern GTK4".
- Decide per-app; document the final set in [`APP_STACK.md`](APP_STACK.md).

### Separate small win (do alongside)
`theme_export` does **not** write `~/.config/gtk-4.0/gtk.css`, so libadwaita apps
currently use libadwaita's *default* dark, NOT Meridian's tokens. Add: write the
`@define-color` named colours to `~/.config/gtk-{3,4}.0/gtk.css` so libadwaita
apps get Meridian's exact palette. (Widget CSS there is ignored by libadwaita;
named colours are honoured.)

## Risks
Double-frame on non-relinquishing CSD apps; libadwaita may refuse SSD; perf
(decoration render is a hot path — CLAUDE.md rule 3); `gtk-decoration-layout`
interactions.

---

## Build / deploy / verify workflow (for the next session)

- **Box:** `ssh meridian-arch` logs in as **root**. The desktop session runs as
  **eduard** (uid 1000). Run session commands as:
  `sudo -u eduard env XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-1 GDK_BACKEND=wayland XDG_CURRENT_DESKTOP=Meridian DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus <cmd>`
- **Build dir on box:** `/root/meridian-build` (a clean mirror synced from this
  repo). Do NOT use `/root/meridian-desktop` (older divergent checkout).
  Sync: `tar czf - --exclude=./target --exclude=./.git --exclude='*/target' . | ssh meridian-arch 'tar xzf - -C /root/meridian-build'`
- **Build:** `cd /root/meridian-build && cargo build --release --workspace`.
  Run it **detached** (`setsid nohup … > /root/build_release.log 2>&1 &`) — the
  ath10k WLAN flaps and drops SSH mid-build; poll the log for `Finished release`.
- **Gate (CLAUDE.md):** `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`,
  `cargo test -p meridian-tokens --test design_guard`.
- **Deploy:** `install -m755 target/release/{meridian,meridian-shell} /usr/local/bin/`
  then `systemctl reboot` (compositor change needs a session restart). Binaries
  backed up under `/root/meridian-bin-backup*`. After reboot the box sits at the
  **greeter** — the user must log in before live checks.
- **Screenshot:** `grim` as eduard → `scp` back → crop with PowerShell
  System.Drawing. `grim` occasionally errors "failed to create display" — retry.

## Repo state at handoff
Branch `freebsd-port`, HEAD `93ed0c2`. All session work committed (CODE_INDEX +
generator, 6 audit fixes, theme generation, GTK app stack, cursor-shape-v1,
icons/cursor migration). **Nothing pushed.** Theme generation lives in
`crates/meridian-shell/src/theme_export.rs` + `crates/meridian-shell/assets/gtk/*.css.in`.
Decoration code: `crates/meridian-compositor/src/decoration/`.
