# Meridian — Refactoring Plan

> **Status 2026-08-19: umgesetzt.** Sämtliche Rust-Quelldateien liegen bei
> höchstens 600 physischen Zeilen. Die Aufteilung ist verhaltensgleich und folgt
> fachlichen Grenzen; `source_size_guard` verhindert neue Überschreitungen.

Derived from the deep audit (2026-06-20). Goal: break the few oversized files
into navigable modules **without behaviour changes**, lowering the cost of every
future edit. This document now records the completed refactor and its seams.

## Guiding principles

1. **Move, don't rewrite.** Every step is a mechanical relocation of `impl`
   blocks / free functions, occasionally bumping a helper from private to
   `pub(super)` / `pub(crate)`. No struct/field reshaping.
2. **Follow the convention already in the tree.** The project already splits one
   concern across many files, each containing its own `impl MeridianShell {}`
   (`state/`, `state/ipc/`, `backend/drm/`, `wayland/handlers/`). Every split
   below mirrors that, so diffs stay mostly mechanical and low-risk.
3. **Green gates are the definition of done** (CLAUDE.md): after each step,
   `cargo test --workspace`, `cargo clippy -- -D warnings`, and
   `cargo test -p meridian-tokens --test design_guard` must stay green.
4. **Update `docs/CODE_INDEX.md`** after each move: `scripts/gen_code_index.sh`.

> ⚠️ **design_guard gotcha:** the guard test scans `crates/*/src/**` and several
> hand-written checks key off file *paths* (e.g. `settings_view.rs`). When colour
> /alpha-bearing code moves out of a guarded file, re-check
> `crates/meridian-tokens/tests/design_guard.rs` so coverage moves with it
> (it currently scans by directory walk, so most moves are fine — but verify).

## Priority order (highest value first)

| Target | Result |
|---|---|
| Shell settings | Widgets, category builders, drawing and IDs live under `settings_view/`. |
| Shell state/render/input | Split by lifecycle, popup, action, surface and pointer responsibility. |
| Compositor | DRM init/render, XWayland, setup, grabs and pointer logic are modularized. |
| Login/lock | Runtime, state, animation, rendering, controls and IPC are separated. |
| Config/IPC/tests | Mutation, TOML output, wallpapers and test groups are separated. |
| Compass/Polkit | Rendering phases and Wayland state/dispatch are separated. |
| Regression gate | `cargo test -p meridian-tokens --test source_size_guard`. |

---

## 1. `settings_view.rs` → `settings_view/` directory

Two responsibilities crammed together: ~28 tiny `impl Widget for X` row/card types,
and one ~1100-line `build_settings_widget_tree` (a giant `match SettingsCategory`).

Proposed module:
```
settings_view/
  mod.rs          # re-exports; shared pub(crate) consts + helpers (with_alpha, fit_text)
  category.rs     # SettingsCategory enum + impl + DESKTOP/SYSTEM ordering tables
  ids.rs          # the ~9 widget-id const tables + default_apps_pick/set ids
  widgets/
    chrome.rs     # HeaderBar, BackButton, SearchField, Sidebar*, dividers
    theme.rs      # ThemeRow, CursorThemeRow
    wallpaper.rs  # WallpaperRow
    sound.rs      # SoundSummaryCard, SoundDeviceRow
    display.rs    # DisplayOutputRow, DisplayMode*, Display{Primary,Cycle}Button
    network.rs    # NetworkProfileRow, WifiRow, BluetoothDeviceRow
    printers.rs   # PrinterRow
    default_apps.rs # DefaultAppCategoryRow
  builders/
    <category>.rs # one `pub(crate) fn build_<category>(...)` per match arm
```
`build_settings_widget_tree` becomes a thin dispatcher calling `builders::*`.
**Risk:** sibling widgets share crate-private layout consts + helpers — hoist them
to `settings_view/mod.rs` (or `layout.rs`) and re-export. The builder arms capture
a lot of `&self`; extracted fns take a small context borrow. Verify design_guard.

## 2. `MeridianShell` impls → `state/`, `dispatch/`, `render/` directories

Do **not** reshape the ~205-field struct (high blast radius). Instead split the
three giant `impl MeridianShell {}` blocks by concern, mirroring the compositor's
`state/` layout:
```
wayland/state/        # struct stays in shell.rs; lifecycle (tick, poll_ipc, apply_ipc_event) in mod.rs
  popups.rs           # toggle_*/close_* (calendar/workspace/network/audio) + OSD
  settings.rs         # open_settings_category, sound/network-from-tray, apply_theme, wallpaper
  launcher.rs         # toggle_launcher, warm_launcher_icons, request/poll_launcher_apps_refresh, pinned
  clicks.rs           # handle_panel_click, handle_workspace_click, SNI handlers
wayland/render/       # group draw_*/unmap_* by surface family
  panel.rs  launcher.rs  popups.rs  modals.rs  osd.rs
wayland/dispatch/     # widget_dispatch.rs split: one file per dispatch_* fn
  settings.rs         # the ~350-line dispatch_settings_action + apply_output_mode_selection
```
**Risk:** low–med. Multiple `impl` blocks across files are native Rust; only
friction is bumping a few private helpers to `pub(super)`.

## 3. `meridian-login/src/main.rs` → modules

```
meridian-login/src/
  drm.rs          # Card + open_display_card + card_drives_a_display
  ui_state.rs     # LoginUiState + enums + apply/auth/tick logic
  security_key.rs # yubikey/smartcard detection + uevent parsers (+ their tests)
  theme.rs        # metro_* palette + load_login_theme   (already partly in visual.rs)
  draw.rs         # draw_* + shadow/stroke helpers
  hit_test.rs     # click_target_at + *_rect builders
  anim.rs         # run_animation + compute_anim_frame + ramp_f32
  main.rs         # slim wiring
```
**Risk:** medium — boot-critical, root-side, hard to integration-test. Move in
small commits; keep the security-key parser unit tests co-located; smoke-test on
real DRM hardware (`docs/HARDWARE_SMOKE.md`) before relying on it.

## 4. `protocols/xwayland.rs` → `protocols/xwayland/`

```
xwayland/
  mod.rs            # start_xwayland + re-exports
  geometry.rs       # rect/clamp/output-shape helpers (+ their unit tests)
  lookup.rs         # find_*/restack/reorder helpers
  decoration_sync.rs# DecorationSyncTarget trait + impls + apply_*_ssd (+ tests)
  handler.rs        # impl XwmHandler / XWaylandShellHandler callbacks
```
**Risk:** low — helpers are free functions; the impls move wholesale.

## 5. Render files (perf-sensitive)

- Shell `wayland/render.rs`: group `draw_*`/`unmap_*` into
  `render/{panel,launcher,popups,modals,osd}.rs`; free helpers to `render/util.rs`.
- DRM `backend/drm/render.rs`: extend the existing `mod layers; mod stack;` with
  `render/blur.rs` (blur pipeline) and `render/capture.rs` (screencopy / thumbnail
  / screenshot + PNG encode).
- **Risk:** medium. These obey CLAUDE.md rule 3 (no heap alloc in the render
  loop) and `docs/PERFORMANCE_RULES.md`. *Move, never rewrite*; preserve
  `#[inline]`; do not introduce clones/allocs; re-run the visual smoke on the box.

> Separately tracked (not splits, but render-loop hygiene the audit surfaced):
> popup/launcher `draw_*` allocate a fresh `vec![0u8; …]` scratch buffer per
> repaint (RENDER-1) and `panel_render_signature` clones Strings/Vecs per draw
> (RENDER-2 valid half). Hoist scratch buffers into reusable fields and replace
> the owned signature with a `u64` hash. Do these *with* the render split.

## 6. De-duplicate pixel helpers (quick win)

`with_alpha` and `blit_rgba_to_argb` are copy-pasted across `settings_view.rs`,
`panel_view.rs`, `app_view.rs` (and a blit variant in DRM). Extract one copy into
`meridian-ui` or a shared `meridian-shell/src/draw/util.rs` and delete the rest.

---

## Out of scope here (tracked elsewhere)

- The runtime fixes from this audit (THEME-1/AUDIO-1/LAUNCH-2/OPEN-2/LOG-1 +
  dead-code removal) are already applied — see `docs/CODE_INDEX.md` § "Recent fixes".
- Remaining audit follow-ups not yet done: AUDIO-3 (read mute via
  `wpctl get-volume`), OPEN-4 (`pick_file_manager` honour the user's
  `inode/directory` default), the unified desktop-entry parser (one parser for
  `launcher.rs` + `default_apps.rs`), and the Dolphin→Gwenview launch behaviour
  (KIO-side, not a Meridian code path).
