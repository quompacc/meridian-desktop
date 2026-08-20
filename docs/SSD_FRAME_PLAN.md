# Plan: Uniform Meridian window frame (SSD) matching the mockups

> **HISTORICAL / DEFERRED (2026-08-19).** Keep the measurements and toolkit
> findings. Do not resume global frame-forcing work ahead of the WebKit vertical
> slice; external applications retain their Wayland/XWayland decoration model.

**Goal:** every app window shows Meridian's own titlebar + window controls,
styled to the then-current dark/light desktop mockups (the historical
`assets/bsd_desktop_mockup*.png` files were renamed to
`assets/arch_desktop_mockup*.png` in the current worktree)
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

## Progress — 2026-06-21 session 2 (Windows dev box, no live verify)

Landed in the tree (uncommitted; the CLAUDE.md gate — `cargo test --workspace`,
`clippy`, `design_guard` — must run on the **box**, as the wayland crates do not
compile on Windows):

- **Separate small win DONE.** `theme_export::export_theme` now also writes the
  substituted GTK templates to `~/.config/gtk-{3,4}.0/gtk.css`, the per-user
  override libadwaita actually loads — so libadwaita apps pick up Meridian's
  `@define-color` palette instead of their default dark. Unit test added
  (`config_gtk_css_carries_libadwaita_named_colours`). `design_guard` green.
- **Step 1 buttons (first pass) DONE — needs on-box screenshot tuning.** The
  glass window-control cluster (`decoration/render/elements.rs`) no longer paints
  coloured zones (was close=red / max=accent / min=accent_alt). All three zones +
  the frosted blur veils now use one neutral tone (`surface_alt`); hover lifts the
  zone with a soft neutral `text` highlight (close **grey, not red**). Glass+blur
  titlebar kept — per user: "Stil wie im Mockup, aber mit Glass und Blur, soll
  zum Rest des Systems passen." Exact alphas/tone to be tuned against the mockup
  on the box.

**Step 2 — tested live 2026-06-21 and DROPPED.** Forcing ServerSide + `GTK_CSD=0`
was built, installed and verified on the box. Result: it does **not** reach the
modern app stack. Both Nemo (GTK3) and Ptyxis (GTK4/libadwaita) **never bind
`xdg-decoration`** (compositor log: `new xdg toplevel` with no `request_mode`/
`new_decoration`; `decoration render: skip … has_ssd=false`), and `GTK_CSD=0` is
ignored once an app sets an explicit HeaderBar. The change was reverted.
`APP_STACK.md` was right. The strategy going forward — two-track (full control for
core apps via SSD/own-build, colour-only integration for the rest) — is in
**[`FRAME_STRATEGY.md`](FRAME_STRATEGY.md)**.

**Step 1 still applies:** the SSD frame styling (titlebar tone/height/rounded
corners + exact button alphas vs the mockup) is the right work for the SSD apps
of track A; remaining tuning needs on-box screenshots.

## Progress — 2026-06-21 session 3 (Phase 1, Windows dev box → Arch box)

- **Resting button chrome stripped to clean glyphs (DONE in tree, deployed).**
  Per the mockup the window controls are flat grey `─□×` glyphs with NO resting
  background. Removed from `decoration/render/elements.rs`: the three frosted
  resting zone fills, the two hairline dividers, the resting frosted-blur veils,
  and the tint-only fallback pill. Only the *hovered* control now lifts — a soft
  neutral wash (frosted via `GlassTitlebarInfo` when `glass_blur` is on), close
  kept grey. The glass+blur pane stays on the titlebar surface itself. The
  `glass_buttons` tuning module lost its now-unused `BASE_FACTOR`/`BASE_CAP`
  (kept `HOVER_*`/`TINT_*`). `glass_divider_alpha` remains a valid token, just
  no longer consumed by the renderer.
- **Default terminal = alacritty (SSD), no code change.** `terminal_program()`/
  `prepare_launch()` already resolve `foot → alacritty → …`; foot isn't
  installed, so alacritty (which live-verified as ServerSide/SSD) is the default.
  foot stays an open option — installing it later auto-promotes it.
- **Built + gated + deployed on the box.** `cargo build --release --workspace`
  green; gate green (design_guard ok, clippy `-D warnings` clean, 29×
  `test result: ok`). New `meridian`/`meridian-shell` installed to
  `/usr/local/bin`, box rebooted.
- **NOT yet visually verified.** The deployed binary that preceded this change
  was stale — it rendered the window controls as a 2×2 grid + ⋮ + × (wrong icon
  set), so the on-box screenshot must be re-taken against the *new* binary. The
  box was at the greeter (no eduard login) when the session ended, so the
  alacritty-vs-mockup screenshot + any height/radius/hover-alpha fine-tuning is
  the **next on-box step**.

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
