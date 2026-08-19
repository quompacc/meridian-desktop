# Meridian — Code Index

> **Migration note (2026-08-19):** this indexes the current native
> implementation. It is the source map for behavior that the WebKit vertical
> slice must preserve, not evidence that the target runtime already exists.

A navigational map of the workspace: **which crate/file/function owns what**, so a
contributor (or an AI assistant) can jump straight to the right place instead of
grepping blind. The codebase has several files >1500 lines and one ~205-field
god-object (`MeridianShell`) whose behaviour is spread across many files — this
index exists to make that navigable.

- The **hand-written** sections below ("Crate map", "Concern map") carry the
  *what does it do* knowledge.
- The **generated** appendix at the bottom (`fn`/`struct`/`enum`/`trait`/`impl`
  with `file:line`) is produced by [`scripts/gen_code_index.sh`](../scripts/gen_code_index.sh)
  and answers *where exactly does it live*. Regenerate after moving code:

  ```sh
  scripts/gen_code_index.sh        # rewrites only the GENERATED region
  git diff docs/CODE_INDEX.md      # review the line-number churn
  ```

  Keep it honest in CI the same way the design tokens are guarded: run the script
  and fail if `git diff --exit-code docs/CODE_INDEX.md` is dirty.

See also: [`ARCHITECTURE.md`](ARCHITECTURE.md) (prose overview),
[`REFACTORING_PLAN.md`](REFACTORING_PLAN.md) (how to break up the big files),
[`meridian_design_manifest.md`](meridian_design_manifest.md) (binding design spec).

---

## Crate map

| Crate | Binary? | Purpose |
|---|---|---|
| `meridian` (root, `src/main.rs`) | bin | Thin entry that boots the compositor. |
| `meridian-compositor` | lib | The Wayland/Smithay compositor: backends (DRM/winit), input, decorations, XWayland, output layout, IPC server, render. |
| `meridian-shell` | bin | The desktop shell: panel, launcher, settings, popups, tray (SNI), notifications, audio/network/bluetooth, default-apps, **theme export**. A `wlr-layer-shell` client. |
| `meridian-login` | bin | Greeter / display manager (root): DRM card probe, PAM auth, UI + animation. |
| `meridian-lock` | bin | Lock screen. |
| `meridian-portal` | bin | `xdg-desktop-portal` backend: FileChooser, Screenshot, Settings (appearance/color-scheme), Access. |
| `meridian-polkit` | bin | Polkit authentication agent. |
| `meridian-config` | lib | Config + theme model (`ThemeConfig`, `ThemeColors`), TOML load/save, keybinds, output layout. |
| `meridian-tokens` | lib | **Single source of truth** for the palette/radius/elevation/interaction design tokens. Hosts the `design_guard` test. |
| `meridian-ui` | lib | Widget/style system (`Theme` is `Copy`), primitives, effects. |
| `meridian-ipc` | lib | Shell↔compositor IPC protocol (`ShellCommand`, events). |
| `meridian-freetype` | lib | Font rasterization glue. |
| `meridian-compass-render` | lib | The compass brand widget (login/bootsplash only). |
| `meridian-boot-common` | lib | Shared boot/splash helpers. |

---

## Concern map — "where does feature X live?"

References are `file::function` (line numbers live in the generated appendix, so
this map survives edits). All shell paths are under `crates/meridian-shell/src/`.

### Shell lifecycle & event loop
- Entry / event loop: `main.rs::main` (single-threaded `calloop`; `event_loop.dispatch` at ~500 ms).
- Session/portal bring-up: `main.rs::activate_user_session` (imports env, starts `meridian-session.target`).
- Per-tick work (clock, battery, **audio re-poll**, **launcher refresh swap-in**): `wayland/state/timers.rs::tick`.
- Shell state struct (the ~205-field god-object): `wayland/state.rs` → `wayland/shell.rs` (`struct MeridianShell`).
- Shell construction / startup polls: `wayland/init.rs`.

### Panel
- Render: `wayland/render.rs::draw_panel`; signature/dedup: `wayland/render.rs` (`panel_render_signature`).
- Widget tree: `panel_view.rs::build_panel_widget_tree`.
- Click handling: `wayland/state/panel_actions.rs::handle_panel_click`.

### Launcher  *(LAUNCH-2 fix)*
- State + desktop-entry scan: `launcher.rs` (`LauncherState`, `DesktopApp::load_system`).
- Open/close + async refresh trigger: `wayland/state/popups.rs::toggle_launcher` → `request_launcher_apps_refresh`.
- Background rescan swap-in: `wayland/state/timers.rs::poll_launcher_apps_refresh` (called from `tick`).
- Render: `wayland/render.rs::draw_launcher`.

### Settings (the 4100-line file)
- View builders / row widgets: `settings_view/content_builders.rs`, `settings_view/content/*.rs` and the widget modules below `settings_view/`.
- Action dispatch: `wayland/handlers/widget_dispatch.rs::dispatch_widget_action` → `dispatch_settings_action`.

### Audio  *(AUDIO-1 fix)*
- Snapshot model + `is_settled`: `audio/mod.rs` (`AudioSnapshot`).
- Linux backend (PipeWire via `wpctl`): `audio/wpctl.rs::snapshot` / `set_volume` / `toggle_mute`.
- FreeBSD backend (`mixer(8)`): `audio/mixer.rs` (cfg-gated, not built on Linux).
- Tray popup: `audio_popup.rs`; popup open/re-poll: `wayland/state/audio_and_network_popups.rs`.

### Theming  *(THEME-1 fix)*
- **Legacy theme export** (kdeglobals / GTK `settings.ini` / gsettings): `theme_export.rs::export_theme`
  — called from `main.rs::main` (startup) and `wayland/state.rs::apply_theme` (live switch).
- Theme model: `meridian-config/src/theme/types/config.rs` (`ThemeConfig`, `ThemeColors`, `appearance_is_light`).
- Portal appearance/color-scheme: `meridian-portal/src/settings.rs`, signal watcher `meridian-portal/src/lib.rs`.

### Default apps / open  *(OPEN-2 fix)*
- MIME index + queries: `default_apps.rs` (`MimeAppIndex`, `query_default`, `desktop_app_dirs`).
- File-manager pick: `default_apps.rs::pick_file_manager`.
- Note: the shell has **no** built-in file-open path; double-click in a file
  manager is that app's job (see `REFACTORING_PLAN.md` / audit notes).

### Tray / notifications / network / bluetooth
- StatusNotifier (SNI) host + menu: `status_notifier.rs`, `status_notifier_popup.rs`.
- Notifications (org.freedesktop.Notifications): `notifications/`.
- Network (NetworkManager `nmcli`, cfg-gated): `network/nmcli.rs`; FreeBSD: `network/freebsd.rs`; popup `network_popup.rs`.
- Bluetooth: `bluetooth.rs`.

### Compositor (`crates/meridian-compositor/src/`)
- State: `state/` (struct in `state/mod.rs`, IPC server `state/ipc/`, output layout `state/output_layout.rs`).
- Input: `input/pointer/` (hover/decoration feedback `input/pointer/mod.rs::update_hover_cursor_feedback` — **LOG-1 fix**), `input/keyboard.rs`.
- Window decorations (SSD): `decoration/` (`DecorationManager`, `clear_hover_buttons_except` — **LOG-1**), render `decoration/render/`.
- Backends: `backend/drm/` (real hardware), `backend/winit/` (nested dev).
- XWayland: `protocols/xwayland.rs`.

---

## Recent fixes — audit 2026-06-20 (branch `freebsd-port`)

| ID | Symptom | Fix location |
|---|---|---|
| THEME-1 | KDE/GTK apps (Gwenview) ignore dark theme | new `theme_export.rs`; wired in `main.rs`, `state.rs::apply_theme` |
| AUDIO-1 | Sound shows muted after boot until tray click | `state.rs::tick` re-polls until `AudioSnapshot::is_settled`; fields in `shell.rs`/`init.rs` |
| LAUNCH-2 | Launcher hitches/hangs on every open | `launcher.rs::toggle` no longer scans; async `request/poll_launcher_apps_refresh` in `state.rs` |
| OPEN-2 | Default apps "not recognized" w/ empty `XDG_DATA_DIRS` | `default_apps.rs::desktop_app_dirs` treats empty as unset |
| LOG-1 | Full-output repaint + INFO log on every hover tick | `decoration/mod.rs::clear_hover_buttons_except` + `input/pointer/mod.rs` (`info!`→`trace!`) |
| cleanup | Dead code | removed `draw_desktop`/`desktop_buffer`, `ui_preview.rs` (test relocated to `icons/mod.rs`), `IconLoader::new` |

---

## Source-size invariant

Every Rust source file is limited to 600 physical lines. The completed module
layout is captured in the generated symbol map below and the rationale remains
in [`REFACTORING_PLAN.md`](REFACTORING_PLAN.md). CI/local verification:

```sh
cargo test -p meridian-tokens --test source_size_guard
```

---

<!-- BEGIN GENERATED: symbol map — regenerate with scripts/gen_code_index.sh -->

_Generated by `scripts/gen_code_index.sh`. Do not edit by hand._

### `meridian-boot-common`

<details><summary><code>crates/meridian-boot-common/src/lib.rs</code> &mdash; 207 lines</summary>

```rust
8:pub struct SocketIdentity
13:pub fn secure_socket_permissions
18:pub fn socket_identity_for_path
26:pub fn cleanup_socket_path
51:pub fn select_boot_mode
71:pub enum Appearance
77:impl Appearance
78:    pub fn is_light
82:    fn as_str
89:    fn parse
102:pub fn read_appearance
106:pub fn read_appearance_from
114:pub fn write_appearance
118:pub fn write_appearance_to
135:    fn unique_test_dir
148:    fn cleanup_removes_original_socket
163:    fn cleanup_rejects_replaced_non_socket
186:    fn parse_defaults_to_dark
193:    fn roundtrip_via_file
```

</details>

### `meridian-compass-render`

<details><summary><code>crates/meridian-compass-render/src/assets.rs</code> &mdash; 7 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compass-render/src/lib.rs</code> &mdash; 407 lines</summary>

```rust
41:pub struct Fonts<'a>
46:impl Fonts<'static>
48:    pub fn quompacc
58:pub enum BuildError
64:impl std::fmt::Display for BuildError
65:    fn fmt
74:impl std::error::Error for BuildError
79:pub enum TextStyle
90:pub struct FrameOpts
110:impl Default for FrameOpts
111:    fn default
124:pub struct Style
153:impl Default for Style
154:    fn default
180:impl Style
183:    pub fn chart
211:pub struct CompassPainter<'a>
219:    pub fn new
231:    pub fn with_style
236:    pub fn style
242:    pub fn style_mut
249:    pub fn render
301:    pub fn north_glow_position
313:    pub fn compass_radius
320:    pub fn glow_base_radius
326:    pub fn render_text_centered
342:    pub fn render_text_left
357:    pub fn measure_text_width
362:    fn text_font_and_size
373:    pub fn render_glow_at
```

</details>

<details><summary><code>crates/meridian-compass-render/src/lib/background_and_scale.rs</code> &mdash; 253 lines</summary>

```rust
8:pub fn needle_angle_deg
23:fn color_with_alpha
32:fn draw_background
77:fn draw_compass_shadow
125:fn draw_meridian_lines
147:fn draw_scale_ring
```

</details>

<details><summary><code>crates/meridian-compass-render/src/lib/rose_and_needle.rs</code> &mdash; 305 lines</summary>

```rust
3:fn draw_sweep_glint
41:fn draw_rose_shadow
85:fn draw_rose
146:fn draw_needle
229:fn draw_needle_glow
266:fn draw_pivot
```

</details>

<details><summary><code>crates/meridian-compass-render/src/lib/text.rs</code> &mdash; 142 lines</summary>

```rust
3:fn draw_signature
9:fn draw_cardinals
28:fn draw_heading_mark
51:struct TextMetrics
57:fn measure_text
85:fn draw_text_at_baseline
129:fn draw_text_centered
```

</details>

<details><summary><code>crates/meridian-compass-render/src/lib_tests.rs</code> &mdash; 234 lines</summary>

```rust
5:fn quompacc_fonts_construct_a_painter
11:fn garbage_sans_font_yields_build_error
23:fn garbage_script_font_yields_build_error
35:fn needle_angle_finite_for_relevant_t_range
43:fn needle_angle_at_settle_is_near_north
50:fn renders_without_panic_at_typical_resolutions
73:fn veil_alpha_255_yields_black_frame
96:fn north_glow_position_consistent_with_needle_angle
116:fn glow_base_radius_matches_internal_geometry
124:fn render_glow_at_alone_lights_up_pixels
141:fn measure_text_width_is_positive_for_non_empty
152:fn render_text_left_returns_pen_past_last_glyph
168:fn watermark_alpha_dims_compass_toward_background
201:fn north_glow_disabled_changes_some_pixels
```

</details>

### `meridian-compositor`

<details><summary><code>crates/meridian-compositor/src/backend/clipped_surface.rs</code> &mdash; 265 lines</summary>

```rust
34:struct ClippingShader
36:fn clip_uniform_names
46:pub fn clip_shader
69:pub struct ClippedSurfaceRenderElement
77:impl ClippedSurfaceRenderElement
78:    pub fn new
150:    fn rounded_corners
180:impl Element for ClippedSurfaceRenderElement
181:    fn id
185:    fn current_commit
189:    fn geometry
193:    fn src
197:    fn transform
201:    fn damage_since
215:    fn opaque_regions
235:    fn alpha
239:    fn kind
244:impl RenderElement<GlesRenderer> for ClippedSurfaceRenderElement
245:    fn draw
262:    fn underlying_storage
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/glass.rs</code> &mdash; 364 lines</summary>

```rust
34:struct GlassShader
35:struct BlurShader
37:fn glass_uniform_names
48:fn blur_uniform_names
53:pub fn glass_shader
74:pub fn blur_shader
98:pub struct GlassTitlebarInfo
120:pub struct GlassTitlebarElement
126:impl GlassTitlebarElement
129:    pub fn new
178:impl Element for GlassTitlebarElement
179:    fn id
182:    fn current_commit
185:    fn geometry
188:    fn src
191:    fn transform
194:    fn damage_since
203:    fn opaque_regions
206:    fn alpha
210:    fn kind
215:impl RenderElement<GlesRenderer> for GlassTitlebarElement
216:    fn draw
239:    fn underlying_storage
253:pub enum GlassElement
262:impl GlassElement
263:    pub fn pending
273:    pub fn pending_info
281:impl Element for GlassElement
282:    fn id
288:    fn current_commit
294:    fn geometry
300:    fn src
306:    fn transform
312:    fn damage_since
322:    fn opaque_regions
328:    fn alpha
334:    fn kind
339:impl RenderElement<GlesRenderer> for GlassElement
340:    fn draw
355:    fn underlying_storage
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/gpu.rs</code> &mdash; 156 lines</summary>

```rust
21:pub
56:fn gpu_candidates
61:fn primary_gpu_candidate
66:fn gpu_candidates
87:fn primary_gpu_candidate
92:fn probe_gpu_connectors
118:fn probe_connected
119:    struct ProbeDrmDevice<'a>
120:    impl AsFd for ProbeDrmDevice<'_>
121:        fn as_fd
125:    impl smithay::reexports::drm::Device for ProbeDrmDevice<'_>
126:    impl smithay::reexports::drm::control::Device for ProbeDrmDevice<'_>
140:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/init.rs</code> &mdash; 596 lines</summary>

```rust
71:struct DrmConnectorReconfigureCandidate
81:struct DrmConnectorChangeSet
88:struct DrmConnectorRemoveCandidate
94:struct PendingInitOutput
108:pub
123:pub fn init_drm
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/init/build.rs</code> &mdash; 57 lines</summary>

```rust
1:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/init/event_sources.rs</code> &mdash; 137 lines</summary>

```rust
1:fn configure_repaint_interval
28:fn register_drm_event_source<Source>
79:fn register_repaint_timer_source
127:fn register_libinput_event_source
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/init/hotplug.rs</code> &mdash; 545 lines</summary>

```rust
1:fn scan_drm_connectors_for_h5b
167:fn add_drm_output_via_hotplug_pipeline
455:fn remove_drm_output_via_hotplug_pipeline
532:fn detach_drm_output
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/init/layout.rs</code> &mdash; 97 lines</summary>

```rust
1:fn sync_primary_flags_from_resolved_layout
37:fn parse_output_scale
50:fn output_scale_from_value
58:fn classify_drm_connector_changes
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/init_diagnostics.rs</code> &mdash; 133 lines</summary>

```rust
12:pub
41:pub
85:pub
98:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/init_env.rs</code> &mdash; 126 lines</summary>

```rust
5:pub
16:pub
20:pub
24:pub
34:pub
50:pub
57:fn duration_from_hz
64:pub
73:pub
124:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/login_ipc.rs</code> &mdash; 77 lines</summary>

```rust
30:pub fn send_handover
47:pub fn send_first_frame
61:fn send_command
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/mod.rs</code> &mdash; 228 lines</summary>

```rust
39:pub type GbmDrmCompositor =
43:pub
63:pub struct DrmOutput
84:pub struct DisabledDrmOutput
92:pub enum DrmCursorIcon
100:pub struct DrmBackend
119:impl DrmBackend
120:    pub fn disable_output
157:    pub fn enable_output_pull_pending
165:    pub fn rebuild_compositor_for_mode
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/mode_selection.rs</code> &mdash; 461 lines</summary>

```rust
9:pub
33:pub
160:pub
219:fn pick_best_mode_for_request
247:fn mode_flags_weird_penalty
258:pub
289:pub
295:pub
307:fn log_selected
318:fn same_mode
329:fn mode_brief
344:fn select_best_mode_for_size
362:fn select_safe_mode
387:fn parse_mode_size
395:pub
404:pub
417:    fn toml_mode_override_picks_matching_size
424:    fn toml_mode_override_picks_closest_refresh_when_specified
435:    fn toml_mode_override_falls_back_when_size_unavailable
442:    fn toml_mode_override_none_delegates_to_select_add_mode
451:    fn toml_mode_override_invalid_dimensions_delegate_to_auto_selection
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/openbsd_privsep.rs</code> &mdash; 102 lines</summary>

```rust
24:pub
28:fn is_allowed_drm_path
95:    fn allows_only_numbered_drm_nodes
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/render.rs</code> &mdash; 495 lines</summary>

```rust
60:fn render_window_toplevel_elements<C>
127:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/render/capture.rs</code> &mdash; 495 lines</summary>

```rust
1:fn serve_screencopy_frames
91:fn process_thumbnail_requests
238:fn process_screenshot_requests
376:fn screenshot_png_bytes
404:fn encode_screenshot_png
421:fn crop_xrgb
434:fn sanitize_window_id
446:fn scale_down_xrgb
467:    fn encodes_xrgb_with_red_blue_swapped
479:    fn dimensions_and_blue_channel_round_trip
492:    fn short_buffer_is_an_error_not_a_panic
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/render/layers.rs</code> &mdash; 229 lines</summary>

```rust
22:struct LayerRenderState
30:impl LayerRenderState
31:    fn mapped
36:fn layer_render_state
74:pub
76:fn is_upper_layer
100:pub
169:pub
197:pub
217:    fn launcher_namespace_forces_upper_bucket
223:    fn non_launcher_uses_layer_role
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/render/scene_composition.rs</code> &mdash; 411 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/render/scene_helpers.rs</code> &mdash; 244 lines</summary>

```rust
1:fn themed_layer_glass_info
30:fn render_scene_for_blur
80:fn is_glass_blur_source_excluded
91:fn first_rendered_blur_source
102:struct PendingGlassBatch
108:fn blur_pass
172:fn blur_scene
192:fn clear_output_dirty
209:fn render_window_popup_elements<C>
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/render/stack.rs</code> &mdash; 113 lines</summary>

```rust
4:pub enum RenderStackRole
13:pub fn layer_role
20:pub fn render_stack_order
63:    fn element_order_is_front_to_back
78:    fn render_stack_without_cursor_has_no_cursor_role
84:    fn render_stack_empty_returns_empty_vec
92:    fn cursor_role_is_always_first_when_present
100:    fn layer_role_overlay_and_top_are_top_layer
106:    fn layer_role_background_and_bottom_are_bottom_layer
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/stats.rs</code> &mdash; 472 lines</summary>

```rust
2:struct DurationStats
9:impl DurationStats
10:    fn record
23:    fn avg_ms
30:    fn min_ms
37:    fn max_ms
46:pub struct DrmTimingStats
81:pub struct PerOutputDirtyStats
91:pub struct DrmDirtyStats
100:impl DrmDirtyStats
101:    pub fn new
116:    pub fn register_output
124:    pub fn unregister_output
132:    pub fn record_dirty_mark_event
140:    pub fn record_dirty_set
150:    pub fn record_dirty_clear
160:    pub fn record_skipped_clean
170:    pub fn record_skipped_power_off
180:    pub fn record_rendered_dirty
190:    pub fn record_rendered_while_not_dirty
200:    pub fn report_if_due
250:impl DrmTimingStats
251:    pub fn new
298:    pub
351:    pub
378:    fn report_if_due
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/wscons.rs</code> &mdash; 508 lines</summary>

```rust
55:struct WsconsEvent
62:enum WsconsDevice
67:impl Device for WsconsDevice
68:    fn id
76:    fn name
84:    fn has_capability
92:    fn usb_id
96:    fn syspath
105:struct WsconsInput;
107:impl InputBackend for WsconsInput
108:    type Device = WsconsDevice;
109:    type KeyboardKeyEvent = WsconsKeyboardEvent;
110:    type PointerAxisEvent = WsconsAxisEvent;
111:    type PointerButtonEvent = WsconsButtonEvent;
112:    type PointerMotionEvent = WsconsMotionEvent;
113:    type PointerMotionAbsoluteEvent = UnusedEvent;
114:    type GestureSwipeBeginEvent = UnusedEvent;
115:    type GestureSwipeUpdateEvent = UnusedEvent;
116:    type GestureSwipeEndEvent = UnusedEvent;
117:    type GesturePinchBeginEvent = UnusedEvent;
118:    type GesturePinchUpdateEvent = UnusedEvent;
119:    type GesturePinchEndEvent = UnusedEvent;
120:    type GestureHoldBeginEvent = UnusedEvent;
121:    type GestureHoldEndEvent = UnusedEvent;
122:    type TouchDownEvent = UnusedEvent;
123:    type TouchUpEvent = UnusedEvent;
124:    type TouchMotionEvent = UnusedEvent;
125:    type TouchCancelEvent = UnusedEvent;
126:    type TouchFrameEvent = UnusedEvent;
127:    type TabletToolAxisEvent = UnusedEvent;
128:    type TabletToolProximityEvent = UnusedEvent;
129:    type TabletToolTipEvent = UnusedEvent;
130:    type TabletToolButtonEvent = UnusedEvent;
131:    type SwitchToggleEvent = UnusedEvent;
132:    type SpecialEvent = UnusedEvent;
136:struct WsconsKeyboardEvent
143:impl Event<WsconsInput> for WsconsKeyboardEvent
144:    fn time
148:    fn device
153:impl KeyboardKeyEvent<WsconsInput> for WsconsKeyboardEvent
154:    fn key_code
160:    fn state
164:    fn count
170:struct WsconsMotionEvent
176:impl Event<WsconsInput> for WsconsMotionEvent
177:    fn time
181:    fn device
186:impl PointerMotionEvent<WsconsInput> for WsconsMotionEvent
187:    fn delta_x
191:    fn delta_y
195:    fn delta_x_unaccel
199:    fn delta_y_unaccel
205:struct WsconsButtonEvent
211:impl Event<WsconsInput> for WsconsButtonEvent
212:    fn time
216:    fn device
221:impl PointerButtonEvent<WsconsInput> for WsconsButtonEvent
222:    fn button_code
226:    fn state
232:struct WsconsAxisEvent
239:impl Event<WsconsInput> for WsconsAxisEvent
240:    fn time
244:    fn device
249:impl PointerAxisEvent<WsconsInput> for WsconsAxisEvent
250:    fn amount
254:    fn amount_v120
258:    fn source
266:    fn relative_direction
271:pub
313:fn device_path
320:fn read_events
358:fn dispatch_keyboard_events
411:fn dispatch_pointer_events
467:fn event_time_micros
475:fn wscons_button_code
489:    fn maps_primary_wscons_buttons_to_linux_codes_expected_by_smithay
497:    fn converts_wscons_timespec_to_microseconds
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/mod.rs</code> &mdash; 3 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/winit/layers.rs</code> &mdash; 119 lines</summary>

```rust
16:pub
18:fn is_upper_layer
42:pub
64:pub
87:pub
107:    fn launcher_namespace_forces_upper_bucket
113:    fn non_launcher_uses_layer_role
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/winit/mod.rs</code> &mdash; 254 lines</summary>

```rust
46:pub
56:pub fn init_winit
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/winit/scene.rs</code> &mdash; 282 lines</summary>

```rust
22:fn render_window_popup_elements<C>
59:fn render_window_toplevel_elements<C>
131:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/cursor/embedded.rs</code> &mdash; 247 lines</summary>

```rust
17:pub
25:fn point_in_polygon
44:fn scaled_arrow_polygon
61:fn point_segment_distance
86:fn min_distance_to_polygon_edges
98:pub
154:type CursorSegment =
156:fn draw_polyline_cursor
186:fn resize_cursor_segments
232:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/cursor/image.rs</code> &mdash; 245 lines</summary>

```rust
18:pub struct CursorImage
28:impl CursorImage
29:    pub fn load_default
43:    pub fn load_theme
48:    pub fn load_theme_icon
120:    pub fn load_theme_cursor_icon
139:    pub fn embedded
143:    pub fn embedded_sized
147:    fn embedded_sized_with_kind
164:    pub fn is_valid_visible_image
185:    pub fn to_memory_buffer
201:fn embedded_kind_for_icon_names
237:fn embedded_name_for_icon_names
```

</details>

<details><summary><code>crates/meridian-compositor/src/cursor/mod.rs</code> &mdash; 8 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/cursor/tests.rs</code> &mdash; 242 lines</summary>

```rust
14:fn env_lock
20:fn embedded_cursor_has_correct_dimensions
31:fn embedded_cursor_is_valid
36:fn empty_theme_returns_embedded
41:fn empty_theme_uses_requested_size
48:fn embedded_cursor_icon_name_is_left_ptr
53:fn empty_theme_resize_icon_requests_return_named_embedded_variants
69:fn embedded_resize_variants_are_visually_distinct_from_default
78:fn embedded_cursor_hotspot_is_origin
87:fn embedded_cursor_pixel_spot_check
112:fn embedded_cursor_is_not_all_transparent
123:fn embedded_cursor_tip_is_opaque
129:fn embedded_cursor_uses_premultiplied_alpha
145:fn embedded_cursor_to_memory_buffer_succeeds
150:fn cursor_has_valid_image
158:fn cursor_theme_loads_successfully
182:fn create_cursor_theme_fixture
199:fn sample_xcursor_file
240:fn push_u32
```

</details>

<details><summary><code>crates/meridian-compositor/src/cursor/xcursor.rs</code> &mdash; 84 lines</summary>

```rust
11:fn build_xcursor_path
22:pub
27:pub
80:fn nearest_image
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/icons.rs</code> &mdash; 181 lines</summary>

```rust
2:pub enum WindowIcon
10:pub enum IconTint
15:type Segment =
39:fn icon_segments
48:fn point_segment_distance
73:fn scale_viewbox_point
83:pub fn rasterize
143:    fn pixel
149:    fn test_rasterize_minimize_returns_correct_size
155:    fn test_rasterize_close_has_pixels_near_diagonals
163:    fn test_rasterize_close_has_transparent_corners
172:    fn test_rasterize_uses_stroke_color
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/mod.rs</code> &mdash; 235 lines</summary>

```rust
34:pub enum DecorationRenderElement
49:pub enum DecorationResizeEdge
61:pub enum DecorationHit
69:pub struct DecorationManager
82:impl DecorationManager
83:    pub fn new
93:    pub
97:    pub fn set_ssd
108:    fn set_hover_and_mark_dirty
119:    pub fn set_focused
128:    pub fn set_maximized
139:    pub fn set_tiled
150:    pub fn set_fullscreen
161:    pub fn update_hover_button
182:    pub fn clear_hover_buttons_except
196:    pub fn remove
200:    pub fn has_ssd
208:impl Default for DecorationManager
209:    fn default
219:    fn update_hover_button_marks_dirty_only_on_state_transition
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/model.rs</code> &mdash; 131 lines</summary>

```rust
5:pub enum HoveredButton
11:pub
23:impl DecorationBuffers
24:    pub
40:pub
53:impl WindowDecoration
54:    pub
69:    pub
73:    pub
77:    pub
87:    pub
91:    pub
100:pub
114:    fn set_hover_reports_transitions_only_when_value_changes
122:    fn clear_hover_returns_true_iff_some_deco_was_hovered
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/render/buffers.rs</code> &mdash; 145 lines</summary>

```rust
11:pub
19:pub
28:pub
125:    fn effective_shadow_alpha_uses_theme_for_focused_window
130:    fn effective_shadow_alpha_drops_to_inactive_when_unfocused
141:    fn effective_shadow_radius_halves_when_unfocused
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/render/elements.rs</code> &mdash; 472 lines</summary>

```rust
24:impl DecorationManager
26:    pub fn render_elements
468:impl From<SolidColorRenderElement> for DecorationRenderElement
469:    fn from
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/render/elements/glass_buttons.rs</code> &mdash; 57 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/render/elements/shaders.rs</code> &mdash; 194 lines</summary>

```rust
1:fn shadow_uniform_names
65:fn rounded_quad_uniform_names
77:fn rounded_quad_element
161:fn glass_uniform_names
174:fn glass_titlebar_element
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/render/geometry.rs</code> &mdash; 418 lines</summary>

```rust
10:pub
22:impl SsdFrameMetrics
23:    pub
56:    pub
69:    pub
76:    pub
88:pub
97:pub
110:pub
114:impl SsdChromeMetrics
115:    pub
119:    pub
123:    pub
127:    pub
158:    pub
214:impl DecorationManager
215:    pub
239:    pub fn decoration_offset
256:    pub fn decoration_inset
282:    pub fn content_corner_radius
299:    fn metrics_from_frame_origin_match_expected_client_and_frame_geometry
315:    fn metrics_from_client_origin_reconstructs_frame_origin
324:    fn zero_titlebar_case_keeps_top_inset_to_border_only
334:    fn button_rects_match_current_render_and_hit_formulas
353:    fn button_rects_are_absent_when_titlebar_is_hidden
360:    fn offset_and_inset_match_existing_formulas_for_common_states
399:    fn resize_bands_and_corners_match_hit_region_edges
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/render/hit.rs</code> &mdash; 274 lines</summary>

```rust
11:pub
28:pub
91:fn point_in_rect
97:impl DecorationManager
98:    pub fn hit_test
151:    fn titlebar_point_hits_titlebar_region
158:    fn titlebar_lower_boundary_is_exclusive
165:    fn resize_top_band_precedence_is_before_titlebar
172:    fn fractional_pointer_coordinates_keep_truncation_behavior_at_titlebar_boundary
181:    fn button_points_hit_close_maximize_and_minimize_regions
198:    fn border_points_hit_left_right_top_and_bottom_regions
219:    fn thin_visual_border_still_has_practical_resize_hit_area
236:    fn border_corner_points_hit_available_resize_corners
253:    fn docked_close_button_owns_top_right_corner
262:    fn client_content_point_hits_client_region
269:    fn point_outside_frame_misses
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/render/icon_cache.rs</code> &mdash; 85 lines</summary>

```rust
11:pub
17:impl IconCache
18:    pub
26:    pub
55:    fn len
67:    fn test_icon_cache_lazy_builds_on_first_request
76:    fn test_icon_cache_returns_same_buffer_on_second_request
```

</details>

<details><summary><code>crates/meridian-compositor/src/decoration/render/mod.rs</code> &mdash; 5 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/grabs/mod.rs</code> &mdash; 2 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/grabs/move_grab.rs</code> &mdash; 273 lines</summary>

```rust
29:pub struct MoveSurfaceGrab
43:impl PointerGrab<MeridianState> for MoveSurfaceGrab
44:    fn motion
129:    fn relative_motion
139:    fn button
182:    fn axis
191:    fn frame
199:    fn gesture_swipe_begin
207:    fn gesture_swipe_update
215:    fn gesture_swipe_end
223:    fn gesture_pinch_begin
231:    fn gesture_pinch_update
239:    fn gesture_pinch_end
247:    fn gesture_hold_begin
255:    fn gesture_hold_end
264:    fn start_data
268:    fn unset
```

</details>

<details><summary><code>crates/meridian-compositor/src/grabs/move_grab/release.rs</code> &mdash; 310 lines</summary>

```rust
1:fn is_pointer_near_output_top_edge
9:fn is_pointer_near_output_left_edge
17:fn is_pointer_near_output_right_edge
27:enum MoveReleaseEdgeAction
32:fn release_edge_action_for_output
48:fn select_move_release_output
55:fn move_release_workarea_geometry
59:fn release_edge_action_on_move_release
70:fn should_maximize_on_move_release
106:fn maximize_window_from_move_release
112:fn apply_half_snap_tiled_states
129:fn apply_half_snap_from_move_release
179:fn select_output_geometry_for_rect_center
190:fn rect_matches_output_fullscreen_shape
200:fn window_is_output_fullscreen_shape
209:fn xwayland_snap_rect_for_action
235:fn apply_xwayland_snap_from_move_release
```

</details>

<details><summary><code>crates/meridian-compositor/src/grabs/move_grab/restore.rs</code> &mdash; 290 lines</summary>

```rust
1:fn half_snap_restore_geometry_source
16:fn movement_crosses_restore_threshold
25:fn restored_initial_window_location
34:fn pointer_ratio_within_frame_x
42:struct DragRestorePointerAnchor
47:fn frame_geometry_from_client
59:fn drag_restore_anchor_from_start_pointer
81:fn anchored_client_location_from_pointer
97:fn anchored_restore_client_location
112:fn maybe_restore_maximized_drag
156:fn window_half_snap_direction
170:fn consume_half_snap_restore_geometry
181:fn apply_half_snap_drag_restore_states
190:fn maybe_restore_half_snapped_drag
241:fn xwayland_restore_window_key
249:fn maybe_restore_xwayland_snapped_drag
```

</details>

<details><summary><code>crates/meridian-compositor/src/grabs/move_grab_tests.rs</code> &mdash; 341 lines</summary>

```rust
18:fn point
23:fn top_edge_threshold_detects_near_top
35:fn top_edge_threshold_rejects_deeper_positions
47:fn top_edge_threshold_requires_pointer_inside_output
59:fn drag_restore_threshold_requires_real_drag_distance
69:fn anchored_restore_location_preserves_pointer_horizontal_ratio
88:fn drag_restore_anchor_clamps_pointer_ratio_near_left_edge
105:fn drag_restore_anchor_clamps_pointer_ratio_near_right_edge
122:fn anchored_restore_location_applies_floating_insets_after_frame_anchor
143:fn left_edge_release_triggers_left_half_snap
159:fn right_edge_release_triggers_right_half_snap
175:fn top_edge_maximize_precedes_side_snap
188:fn release_away_from_edges_does_not_snap
203:fn move_release_workarea_subtracts_panel_reservation
218:fn half_snap_tiled_states_are_set_for_left_and_right
235:fn half_snap_restore_prefers_maximize_restore_geometry
255:fn consume_half_snap_restore_geometry_prefers_and_consumes_stored_entry
275:fn consume_half_snap_restore_geometry_falls_back_to_current_geometry
289:fn half_snap_drag_restore_clears_tiled_bits_and_preserves_other_states
309:fn xwayland_snap_rect_for_maximize_uses_workarea_geometry
322:fn xwayland_snap_rect_for_half_snap_splits_width
```

</details>

<details><summary><code>crates/meridian-compositor/src/grabs/resize_grab/grab.rs</code> &mdash; 284 lines</summary>

```rust
23:enum ResizeSurfaceTarget
28:pub struct ResizeSurfaceGrab
36:impl ResizeSurfaceGrab
37:    pub fn start
66:impl PointerGrab<MeridianState> for ResizeSurfaceGrab
67:    fn motion
147:    fn relative_motion
157:    fn button
200:    fn axis
208:    fn frame
215:    fn gesture_swipe_begin
223:    fn gesture_swipe_update
231:    fn gesture_swipe_end
239:    fn gesture_pinch_begin
247:    fn gesture_pinch_update
255:    fn gesture_pinch_end
263:    fn gesture_hold_begin
271:    fn gesture_hold_end
280:    fn start_data
283:    fn unset
```

</details>

<details><summary><code>crates/meridian-compositor/src/grabs/resize_grab/mod.rs</code> &mdash; 28 lines</summary>

```rust
12:    pub struct ResizeEdge: u32
24:impl From<xdg_toplevel::ResizeEdge> for ResizeEdge
25:    fn from
```

</details>

<details><summary><code>crates/meridian-compositor/src/grabs/resize_grab/state.rs</code> &mdash; 91 lines</summary>

```rust
13:pub
26:impl ResizeSurfaceState
27:    pub
37:    fn commit
55:pub fn handle_commit
```

</details>

<details><summary><code>crates/meridian-compositor/src/input/keyboard.rs</code> &mdash; 296 lines</summary>

```rust
14:struct KeyMatch
19:fn wm_split_dir
26:fn focused_window_for_close
42:pub fn handle_keyboard<I: InputBackend>
229:fn workspace_idx_from_digit_keysym
244:fn is_workspace_fallback_shortcut
255:fn is_audio_key
264:fn audio_key_event
279:    fn audio_keys_map_to_events
```

</details>

<details><summary><code>crates/meridian-compositor/src/input/mod.rs</code> &mdash; 54 lines</summary>

```rust
8:impl MeridianState
9:    pub fn process_input_event<I: InputBackend>
```

</details>

<details><summary><code>crates/meridian-compositor/src/input/pointer/button.rs</code> &mdash; 569 lines</summary>

```rust
33:pub fn handle_pointer_button<I: InputBackend>
```

</details>

<details><summary><code>crates/meridian-compositor/src/input/pointer/button/helpers.rs</code> &mdash; 190 lines</summary>

```rust
1:type HitInfo =
8:fn decoration_hit_info
66:fn select_pointer_button_output_info
90:fn surface_belongs_to_layer
105:fn xwayland_override_redirect_window_under_pointer
127:fn started_move_grab_window_states
143:fn decoration_resize_edge_to_resize_edge
156:fn raise_window_and_focus
172:fn send_pointer_button
```

</details>

<details><summary><code>crates/meridian-compositor/src/input/pointer/button_tests.rs</code> &mdash; 146 lines</summary>

```rust
6:fn click_point_on_output_one
44:fn click_point_on_output_two
82:fn outside_point_uses_primary_fallback
104:fn no_primary_uses_first_fallback
142:fn empty_registry_is_safe
```

</details>

<details><summary><code>crates/meridian-compositor/src/input/pointer/mod.rs</code> &mdash; 496 lines</summary>

```rust
25:pub fn handle_pointer_motion_absolute<I: InputBackend>
78:pub fn handle_pointer_motion_relative<I: InputBackend>
173:fn output_geometry_for_rect_center
185:fn rect_matches_output_fullscreen_shape
195:fn xwayland_resize_edge_from_rect
234:pub
260:fn decoration_hit_for_pointer
294:fn cursor_icon_for_decoration_hit
308:fn cursor_icon_for_resize_edge
312:fn update_hover_cursor_feedback
395:fn desktop_bounds
413:fn clamp_point_to_desktop_bounds
432:fn select_output_from_registry_for_point
441:pub
449:pub fn handle_pointer_axis<I: InputBackend>
```

</details>

<details><summary><code>crates/meridian-compositor/src/input/pointer/mod_tests.rs</code> &mdash; 192 lines</summary>

```rust
5:fn reg
21:fn absolute_point_selects_output_one
31:fn absolute_point_selects_output_two
41:fn resize_hit_maps_to_expected_cursor_icons
93:fn non_resize_hit_maps_to_default_cursor_icon
105:fn absolute_point_outside_uses_primary_fallback
116:fn focus_update_candidate_is_none_outside_outputs
124:fn relative_clamp_keeps_point_inside_bounds
134:fn relative_clamp_noop_when_inside_bounds
143:fn xwayland_edge_hit_detects_corners_and_edges
181:fn xwayland_edge_hit_ignores_interior_and_outside_points
```

</details>

<details><summary><code>crates/meridian-compositor/src/lib.rs</code> &mdash; 9 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/protocols/mod.rs</code> &mdash; 2 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/protocols/xdg_shell.rs</code> &mdash; 47 lines</summary>

```rust
9:pub fn handle_commit
```

</details>

<details><summary><code>crates/meridian-compositor/src/protocols/xwayland.rs</code> &mdash; 375 lines</summary>

```rust
43:trait DecorationSyncTarget
44:    fn set_ssd
45:    fn set_focused
46:    fn set_maximized
47:    fn set_fullscreen
50:struct SurfaceDecorationSyncTarget<'a>
55:impl DecorationSyncTarget for SurfaceDecorationSyncTarget<'_>
56:    fn set_ssd
60:    fn set_focused
65:    fn set_maximized
70:    fn set_fullscreen
76:fn apply_managed_map_ssd
87:fn apply_override_redirect_ssd
91:pub
109:pub
169:struct X11Remeasure
181:pub
246:pub
282:pub fn start_xwayland
328:impl XWaylandShellHandler for MeridianState
329:    fn xwayland_shell_state
333:    fn surface_associated
366:impl XwmHandler for MeridianState
```

</details>

<details><summary><code>crates/meridian-compositor/src/protocols/xwayland/configure.rs</code> &mdash; 301 lines</summary>

```rust
3:    fn configure_request
209:    fn configure_notify
```

</details>

<details><summary><code>crates/meridian-compositor/src/protocols/xwayland/helpers.rs</code> &mdash; 273 lines</summary>

```rust
1:fn select_output_geometry_for_rect
13:fn rect_matches_output_fullscreen_shape
23:fn panel_safe_normal_xwayland_rect
49:fn configure_request_rect
72:fn adjusted_configure_request_rect
86:pub
93:pub
119:fn find_active_x11_window
129:fn find_x11_window_with_workspace
141:fn find_x11_surface_by_window_id
154:fn find_x11_window_by_stacking_id
174:fn restack_override_redirect_above_hint
213:fn reorder_above_hint
220:fn update_or_diag_entry<F>
229:fn window_is_output_fullscreen_shape
239:fn x11_resize_edge_to_resize_edge
252:pub
256:fn x11_fullscreen_restore_key
260:fn maximized_x11_content_size
```

</details>

<details><summary><code>crates/meridian-compositor/src/protocols/xwayland/input_selection.rs</code> &mdash; 267 lines</summary>

```rust
3:    fn property_notify
9:    fn resize_request
105:    fn move_request
185:    fn allow_selection_access
209:    fn new_selection
220:    fn cleared_selection
235:    fn send_selection
262:    fn disconnected
```

</details>

<details><summary><code>crates/meridian-compositor/src/protocols/xwayland/state_requests.rs</code> &mdash; 243 lines</summary>

```rust
3:        fn maximize_request
7:        fn unmaximize_request
11:        fn fullscreen_request
61:        fn unfullscreen_request
99:        fn minimize_request
184:        fn unminimize_request
```

</details>

<details><summary><code>crates/meridian-compositor/src/protocols/xwayland/window_lifecycle.rs</code> &mdash; 318 lines</summary>

```rust
3:    fn xwm_state
9:    fn new_window
19:    fn new_override_redirect_window
78:    fn map_window_request
150:    fn map_window_notify
152:    fn mapped_override_redirect_window
207:    fn unmapped_window
270:    fn destroyed_window
```

</details>

<details><summary><code>crates/meridian-compositor/src/protocols/xwayland_tests.rs</code> &mdash; 261 lines</summary>

```rust
12:struct MockDecorationSyncTarget
19:impl DecorationSyncTarget for MockDecorationSyncTarget
20:    fn set_ssd
24:    fn set_focused
28:    fn set_maximized
32:    fn set_fullscreen
38:fn normal_xwayland_rect_is_clamped_to_panel_safe_bottom
56:fn output_sized_rect_is_treated_as_fullscreen_and_left_unchanged
74:fn configure_request_rect_uses_requested_x_y_when_present
84:fn override_redirect_configure_bypasses_panel_clamp
97:fn managed_configure_still_clamps_to_panel_safe_workarea
114:fn override_redirect_always_passes_through
129:fn managed_normal_window_passes_through
140:fn managed_maximized_request_matching_workarea_is_deny_noop
150:fn managed_maximized_request_with_other_rect_is_implicit_unmaximize
161:fn managed_maximized_request_with_same_size_but_other_loc_is_implicit_unmaximize
172:fn managed_fullscreen_request_matching_output_is_deny_noop
182:fn managed_fullscreen_request_with_other_rect_is_implicit_unfullscreen
193:fn managed_fullscreen_takes_priority_over_maximized
204:fn decoration_state_sync_after_map_managed
214:fn decoration_state_sync_for_override_redirect_is_no_ssd
227:fn apply_managed_map_ssd_is_idempotent_when_called_twice
238:fn apply_override_redirect_ssd_overrides_prior_managed_state
246:fn maximized_x11_content_size_subtracts_frame_insets_in_both_axes
252:fn maximized_x11_content_size_clamps_minimum_dimension_to_one
258:fn maximized_x11_content_size_with_zero_decoration_offset_equals_workarea
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/client.rs</code> &mdash; 13 lines</summary>

```rust
5:pub struct ClientState
9:impl ClientData for ClientState
10:    fn initialized
12:    fn disconnected
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/core/compositor.rs</code> &mdash; 246 lines</summary>

```rust
24:impl BufferHandler for MeridianState
25:    fn buffer_destroyed
28:impl CompositorHandler for MeridianState
29:    fn compositor_state
33:    fn client_compositor_state<'a>
40:    fn commit
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/core/grab.rs</code> &mdash; 24 lines</summary>

```rust
9:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/core/layer_shell.rs</code> &mdash; 341 lines</summary>

```rust
17:fn select_layer_output_info<'a>
39:fn select_layer_recovery_output_info<'a>
54:impl MeridianState
55:    pub fn reconcile_layer_shell_outputs_after_output_change
139:impl WlrLayerShellHandler for MeridianState
140:    fn shell_state
144:    fn new_layer_surface
224:    fn new_popup
228:    fn layer_destroyed
255:    fn info
273:    fn explicit_output_wins
281:    fn unknown_requested_output_falls_back_to_primary
290:    fn primary_fallback_without_request
298:    fn first_fallback_without_primary
306:    fn empty_registry_is_safe
311:    fn recovery_lost_output_falls_back_to_primary
320:    fn recovery_lost_output_falls_back_to_first_without_primary
329:    fn recovery_no_outputs_is_safe_none
334:    fn recovery_reconfigure_keeps_same_output_assignment
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/core/mod.rs</code> &mdash; 6 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/core/shm.rs</code> &mdash; 9 lines</summary>

```rust
5:impl ShmHandler for MeridianState
6:    fn shm_state
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/dmabuf.rs</code> &mdash; 37 lines</summary>

```rust
8:impl DmabufHandler for MeridianState
9:    fn dmabuf_state
13:    fn dmabuf_imported
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/idle.rs</code> &mdash; 34 lines</summary>

```rust
11:impl IdleNotifierHandler for MeridianState
12:    fn idle_notifier_state
17:impl IdleInhibitHandler for MeridianState
18:    fn inhibit
28:    fn uninhibit
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/misc.rs</code> &mdash; 355 lines</summary>

```rust
26:fn clamp_client_loc_for_visible_frame
70:fn find_mapped_xdg_window
89:fn reposition_xdg_window_for_visible_frame
131:impl SeatHandler for MeridianState
132:    type KeyboardFocus = WlSurface;
133:    type PointerFocus = WlSurface;
134:    type TouchFocus = WlSurface;
136:    fn seat_state
140:    fn focus_changed
147:    fn cursor_image
156:impl smithay::wayland::tablet_manager::TabletSeatHandler for MeridianState
158:impl OutputHandler for MeridianState
160:impl SelectionHandler for MeridianState
161:    type SelectionUserData =
163:    fn new_selection
180:    fn send_selection
200:impl PrimarySelectionHandler for MeridianState
201:    fn primary_selection_state
206:impl WaylandDndGrabHandler for MeridianState
208:impl DataDeviceHandler for MeridianState
209:    fn data_device_state
214:impl DndGrabHandler for MeridianState
216:impl XdgDecorationHandler for MeridianState
217:    fn new_decoration
235:    fn request_mode
257:    fn unset_mode
284:    fn moves_client_down_when_frame_top_would_be_offscreen
293:    fn moves_client_right_when_left_border_would_be_offscreen
303:    fn keeps_location_when_frame_is_already_fully_visible
312:    fn oversized_window_keeps_top_left_reachable
321:impl MeridianState
322:    pub fn update_focus_decoration
331:    pub fn set_keyboard_focus_with_decorations
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/mod.rs</code> &mdash; 11 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/output_power.rs</code> &mdash; 202 lines</summary>

```rust
14:pub struct OutputPowerData
18:impl GlobalDispatch<ZwlrOutputPowerManagerV1,
19:    fn bind
31:impl Dispatch<ZwlrOutputPowerManagerV1,
32:    fn request
81:impl Dispatch<ZwlrOutputPowerV1, OutputPowerData> for MeridianState
82:    fn request
170:    fn destroyed
189:fn power_mode_to_wire
196:fn power_mode_from_wire
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/screencopy.rs</code> &mdash; 107 lines</summary>

```rust
19:impl ImageCaptureSourceHandler for MeridianState
20:    fn source_destroyed
23:impl OutputCaptureSourceHandler for MeridianState
24:    fn output_capture_source_state
28:    fn output_source_created
33:impl ImageCopyCaptureHandler for MeridianState
34:    fn image_copy_capture_state
38:    fn capture_constraints
48:    fn new_session
57:    fn session_destroyed
61:    fn frame
86:    fn screencopy_constraints_uses_current_mode
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/session_lock.rs</code> &mdash; 86 lines</summary>

```rust
11:impl SessionLockHandler for MeridianState
12:    fn lock_state
16:    fn lock
49:    fn unlock
58:    fn new_surface
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/syncobj.rs</code> &mdash; 9 lines</summary>

```rust
5:impl DrmSyncobjHandler for MeridianState
6:    fn drm_syncobj_state
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/wayland_extra.rs</code> &mdash; 53 lines</summary>

```rust
15:impl XdgActivationHandler for MeridianState
16:    fn activation_state
20:    fn token_created
29:    fn request_activation
39:impl FractionalScaleHandler for MeridianState
40:    fn new_fractional_scale
43:impl InputMethodHandler for MeridianState
44:    fn new_popup
46:    fn popup_repositioned
48:    fn dismiss_popup
50:    fn parent_geometry
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/xdg/lifecycle.rs</code> &mdash; 120 lines</summary>

```rust
15:fn initial_maximized_client_origin
37:pub
100:pub
109:pub
117:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/xdg/mod.rs</code> &mdash; 320 lines</summary>

```rust
22:fn popup_parent_window_loc_and_size
40:fn popup_parent_workarea
58:pub
74:pub
91:impl XdgShellHandler for MeridianState
92:    fn xdg_shell_state
96:    fn new_toplevel
100:    fn new_popup
104:    fn toplevel_destroyed
108:    fn app_id_changed
112:    fn title_changed
116:    fn grab
165:    fn reposition_request
178:    fn move_request
182:    fn resize_request
192:    fn maximize_request
196:    fn unmaximize_request
200:    fn fullscreen_request
204:    fn unfullscreen_request
208:    fn minimize_request
225:    fn make_positioner
243:    fn unconstrain_slides_left_when_popup_overflows_right_edge
264:    fn unconstrain_flips_up_when_popup_overflows_bottom_edge
285:    fn unconstrain_keeps_geometry_when_inside_workarea
303:    fn unconstrain_resizes_when_popup_is_larger_than_workarea
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/xdg/requests/grab.rs</code> &mdash; 113 lines</summary>

```rust
21:pub
71:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/xdg/requests/mod.rs</code> &mdash; 9 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/xdg/requests/state.rs</code> &mdash; 432 lines</summary>

```rust
16:pub
65:fn normal_maximize_frame_for_output
78:fn remeasured_maximized_frame
91:type MaximizedUpdate =
93:pub
137:pub
174:pub
206:pub
217:pub
290:struct SelectedOutput
297:fn select_output_for_surface
312:fn select_output_from_infos_for_point
356:    fn remeasured_maximized_frame_tracks_output_and_skips_noop
380:    fn info
398:    fn selects_primary_on_fallback
406:    fn selects_first_when_no_primary_marked
415:    fn empty_infos_is_safe
420:    fn normal_maximize_frame_uses_panel_safe_height
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/xdg/requests/window.rs</code> &mdash; 19 lines</summary>

```rust
5:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/idle.rs</code> &mdash; 115 lines</summary>

```rust
11:pub struct IdleInhibitorSet<K>
16:    pub fn new
22:    pub fn is_inhibited
26:    pub fn len
30:    pub fn is_empty
36:    pub fn add
44:    pub fn remove
55:    fn new_default_is_not_inhibited
63:    fn first_add_transitions_to_inhibited_returns_true
72:    fn second_add_does_not_transition_returns_false
81:    fn remove_non_last_returns_false_still_inhibited
91:    fn remove_last_returns_true_not_inhibited
100:    fn remove_unknown_key_returns_false_no_transition
108:    fn add_existing_key_returns_false
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/ipc/broadcast.rs</code> &mdash; 290 lines</summary>

```rust
13:fn build_output_workspace_snapshot
54:impl MeridianState
55:    pub fn broadcast_workspace
61:    pub fn broadcast_window_snapshot
109:    pub fn broadcast_output_workspace_changed
128:    pub fn broadcast_output_workspace_snapshot
157:    pub fn broadcast_toplevel_opened
167:    pub fn broadcast_window_opened
171:    pub fn broadcast_toplevel_closed
178:    pub fn broadcast_window_closed
182:    pub fn broadcast_toplevel_focused
199:    pub fn broadcast_toplevel_focus_cleared
203:    pub fn broadcast_toggle_launcher
207:    pub fn broadcast_desktop_context_menu
220:    fn reg
236:    fn output_workspace_snapshot_for_two_outputs_sets_flags
279:    fn output_workspace_snapshot_empty_registry_is_safe
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/ipc/commands.rs</code> &mdash; 549 lines</summary>

```rust
14:impl MeridianState
15:    pub fn poll_ipc
99:    fn handle_shell_command
192:    fn reject_pending_screenshot_consent_without_shell
211:    fn reject_pending_screenshot_region_without_shell
234:    fn resolve_screenshot_region
283:    fn resolve_screenshot_consent
314:    pub fn reload_config
371:    fn reload_cursor_runtime
404:    pub fn focus_window_by_id
496:    pub fn spawn_lock_screen
511:fn spawn_and_reap_launch
542:fn is_firefox_program
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/ipc/conversions.rs</code> &mdash; 30 lines</summary>

```rust
4:pub
8:pub
17:    fn ipc_workspace_to_index_clamps_and_normalizes
26:    fn index_to_legacy_ipc_workspace_is_one_based
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/ipc/launch.rs</code> &mdash; 212 lines</summary>

```rust
3:pub
8:pub
50:fn command_exists
66:fn is_executable_file
98:    fn env_lock
103:    fn with_env_vars<R>
131:    fn non_terminal_launch_keeps_program_and_args
139:    fn empty_program_is_rejected
145:    fn terminal_env_non_executable_file_is_rejected
178:    fn terminal_env_executable_file_is_accepted
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/ipc/mod.rs</code> &mdash; 9 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/ipc/screenshot.rs</code> &mdash; 228 lines</summary>

```rust
10:pub
25:pub
58:pub
79:    fn expect_respond
92:    fn is_await_consent
96:    fn is_await_region_pick
101:    fn invalid_request_is_rejected
129:    fn portal_request_awaits_consent
154:    fn region_request_with_nonzero_size_follows_consent_path
185:    fn portal_interactive_request_awaits_region_pick
208:    fn nonzero_client_id_is_forwarded_to_policy_context
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/ipc/screenshot_policy.rs</code> &mdash; 296 lines</summary>

```rust
3:pub
10:pub
16:pub
21:pub
37:impl ScreenshotPolicy
38:    pub
124:fn internal_capture_dev_enabled
131:pub
146:    fn valid_request
164:    fn portal_request_needs_consent_not_auto_allowed
174:    fn unknown_origin_is_denied
184:    fn internal_origin_allowed_only_with_dev_flag
207:    fn internal_origin_with_invalid_request_still_rejected
225:    fn portal_interactive_request_needs_region_pick
235:    fn region_request_with_nonzero_size_needs_consent
252:    fn region_request_with_zero_dimension_is_invalid
272:    fn invalid_request_is_invalid
285:    fn unknown_requester_via_portal_still_needs_consent
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/ipc/server.rs</code> &mdash; 474 lines</summary>

```rust
20:pub struct IpcServer
29:pub struct IpcPoll
35:pub struct ScreenshotBridgeRequestEnvelope
40:struct IpcClient
49:enum IpcClientRole
55:struct SocketIdentity
60:impl IpcServer
61:    pub fn new
101:    pub fn auth_token
105:    pub fn poll
238:    pub fn broadcast
260:    pub fn send_screenshot_bridge_response
292:    fn retain_alive
302:impl Drop for IpcServer
303:    fn drop
344:fn is_allowed_ipc_peer
368:fn current_effective_uid
373:fn is_same_uid
378:fn peer_effective_uid
412:fn peer_effective_uid
434:fn peer_effective_uid
438:fn socket_identity_for_path
446:fn should_cleanup_socket_path
457:fn generate_ipc_auth_token
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/ipc/server_tests.rs</code> &mdash; 219 lines</summary>

```rust
14:fn same_uid_is_allowed
19:fn different_uid_is_rejected
39:fn peer_uid_of_socketpair_matches_own_euid
47:fn env_lock
52:fn temp_runtime_dir
65:fn with_runtime_dir<R>
77:fn connect_client
81:fn write_command
87:fn unauthenticated_control_command_is_ignored
102:fn authenticated_shell_control_command_is_accepted
124:fn broadcasts_only_reach_authenticated_shell_clients
166:fn cleanup_check_matches_original_socket_identity
189:fn cleanup_check_rejects_replaced_non_socket_path
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/layout/focus.rs</code> &mdash; 50 lines</summary>

```rust
8:impl MeridianState
9:    pub fn focused_window
23:    pub fn move_focused_window_to_workspace
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/layout/mod.rs</code> &mdash; 4 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/layout/surface.rs</code> &mdash; 245 lines</summary>

```rust
11:fn select_surface_output_info
35:impl MeridianState
36:    pub fn surface_under
149:    fn reg
165:    fn point_on_output_one_is_selected
177:    fn point_on_output_two_is_selected
189:    fn outside_point_uses_primary_fallback
201:    fn first_fallback_is_used_when_no_primary_exists
239:    fn empty_registry_is_safe
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/layout/tiling.rs</code> &mdash; 160 lines</summary>

```rust
13:impl MeridianState
14:    pub fn tile_workspace
65:    pub fn toggle_tiling
78:struct SelectedTilingOutput
85:fn output_geometry_to_rect
93:fn select_tiling_output_from_infos
119:    fn info
137:    fn tiling_selects_primary_output
149:    fn tiling_selects_first_when_no_primary_exists
157:    fn tiling_handles_empty_infos
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/layout/workspace.rs</code> &mdash; 457 lines</summary>

```rust
9:enum MoveRequestGuard
15:fn validate_workspace_move_request
32:impl MeridianState
33:    pub fn current_workspace_index_for_focused_output
37:    pub fn current_workspace_index
41:    pub fn focused_output
46:    pub fn set_focused_output
62:    pub fn active_workspace_for_output
70:    pub fn set_active_workspace_for_output
86:    pub fn sync_outputs_with_workspace_state
121:    pub fn update_focused_output_from_point
157:    pub fn update_focused_output_from_surface
203:    pub fn switch_workspace
246:    pub fn switch_workspace_for_focused_output
304:    pub fn move_focused_window_to_workspace_consistent
430:    fn move_request_invalid_target_is_ignored
438:    fn move_request_without_focused_window_is_ignored
446:    fn move_request_target_equal_source_is_ignored
454:    fn move_request_with_valid_target_and_source_is_accepted
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/lock.rs</code> &mdash; 317 lines</summary>

```rust
11:pub enum LockPhase
22:pub struct LockManager
29:impl LockManager
30:    pub fn new
34:    pub fn phase
38:    pub fn is_locked_or_pending
42:    pub fn surface_count
50:    pub fn begin_lock_with_targets
69:    pub fn confirm_locked
78:    pub fn unlock
88:    pub fn record_pending_frame
101:    pub fn forget_pending_target
113:    pub fn pending_target_count
117:    pub fn has_pending_locker
122:    pub fn register_surface
128:    pub fn surface_for_output
134:    pub fn surfaces_iter
141:    pub fn prune_dead_surfaces
148:    pub fn drop_surface
154:impl LockManager
157:    pub fn begin_pending_for_test
168:impl MeridianState
169:    pub fn refresh_lock_focus
207:    fn default_is_unlocked
215:    fn begin_lock_from_unlocked_transitions_to_pending
223:    fn begin_lock_from_pending_or_locked_is_noop
234:    fn confirm_locked_only_from_pending
243:    fn unlock_from_locked_clears_surfaces
254:    fn full_lifecycle_unlocked_pending_locked_unlocked
266:    fn begin_pending_with_targets_keeps_phase_pending_until_all_frames
278:    fn record_pending_frame_unknown_output_is_noop
287:    fn record_pending_frame_last_target_returns_ready_signal
297:    fn forget_pending_target_drains_just_like_record
307:    fn unlock_during_pending_clears_targets_and_locker_state
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/mod.rs</code> &mdash; 510 lines</summary>

```rust
88:pub struct MaximizeRestoreGeometry
93:impl MaximizeRestoreGeometry
94:    pub fn new
103:pub enum HalfSnapDirection
109:pub enum WindowSnapState
114:pub struct HalfSnapRestoreGeometry
119:impl HalfSnapRestoreGeometry
120:    pub fn new
129:pub struct HalfSnapPlacement
135:pub struct MinimizedWindowEntry
142:pub struct XwaylandOrDiagConfigureRequest
155:pub struct XwaylandOrDiagConfigureNotify
162:pub struct XwaylandOrDiagPointerEvent
171:pub struct XwaylandOrDiagReleaseCandidate
181:pub struct XwaylandOrDiagReleaseState
195:pub struct XwaylandOrDiagEntry
215:pub
223:pub
230:pub
237:pub
255:pub
266:pub
281:pub
308:pub
320:pub
328:pub struct ThumbnailRequest
337:pub struct PendingScreenshotRequest
342:pub struct MeridianState
430:impl MeridianState
431:    pub fn resolve_output_layout
437:    pub
460:    pub fn clear_window_runtime_state
467:    pub fn keyboard_focus_diag_target
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/mod_tests.rs</code> &mdash; 236 lines</summary>

```rust
12:fn capture_known_loc_and_size_preserves_client_size
25:fn premaximized_entry_allows_missing_client_size
37:fn existing_entry_is_not_overwritten
48:fn missing_restore_entry_uses_fallback_location
55:fn maximize_mapping_adds_decoration_offset_to_output_origin
62:fn normal_window_workarea_subtracts_bottom_panel_reservation
80:fn normal_window_workarea_rect_preserves_origin_and_width
92:fn unmaximize_restore_uses_stored_geometry_without_fallback
103:fn unmaximize_restore_uses_decoration_offset_when_missing
110:fn clear_tiled_toplevel_states_unsets_only_tiled_bits
130:fn half_snap_left_placement_uses_left_output_half
148:fn half_snap_right_placement_uses_right_output_half
166:fn half_snap_nonzero_output_origin_is_preserved
184:fn half_snap_placement_applies_ssd_offset_and_inset
211:fn half_snap_odd_output_width_assigns_extra_pixel_to_right_half
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_hotplug_tests.rs</code> &mdash; 542 lines</summary>

```rust
40:struct TestSpaceElement
47:impl TestSpaceElement
48:    fn new
57:    fn entered
62:impl PartialEq for TestSpaceElement
63:    fn eq
68:impl Eq for TestSpaceElement
70:impl Hash for TestSpaceElement
71:    fn hash<H: Hasher>
76:impl IsAlive for TestSpaceElement
77:    fn alive
82:impl SpaceElement for TestSpaceElement
83:    fn bbox
87:    fn is_in_input_region
91:    fn set_activate
93:    fn output_enter
97:    fn output_leave
102:fn make_output
125:struct OutputHotplugFixture
135:impl OutputHotplugFixture
136:    fn new
150:    fn with_layout_from_entries
164:    fn sync_smithay_output_state
186:    fn add_output
262:    fn remove_output
281:    fn reconfigure_output
344:    fn reload_layout_from_entries
437:    fn simulate_disable_output
464:    fn map_window
479:    fn window_location
487:    fn window_count
491:    fn move_window_between_workspaces
512:    fn refresh_all_spaces
518:    fn snapshot
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_hotplug_tests/basic.rs</code> &mdash; 253 lines</summary>

```rust
2:fn single_output_add_yields_zero_origin_primary
12:fn two_outputs_default_layout_chains_horizontally
24:fn remove_primary_falls_back_to_remaining
36:fn add_remove_add_is_idempotent
50:fn remove_unknown_output_is_safe_noop
61:fn reconfigure_changes_width_keeps_chain_after
74:fn reconfigure_unknown_output_is_safe_noop
81:fn reconfigure_same_size_is_stable
90:fn layout_with_explicit_primary_overrides_first_default
103:fn layout_right_of_chain_three_outputs
131:fn layout_below_chain_two_outputs
149:fn layout_coord_position_pins_exact_xy
167:fn layout_dangling_reference_falls_back_to_auto
185:fn safety_net_triggers_when_all_outputs_disabled_in_layout
201:fn cyclic_add_remove_stability
220:fn four_outputs_complex_layout_snapshot
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_hotplug_tests/reload.rs</code> &mdash; 179 lines</summary>

```rust
2:fn reload_layout_changes_primary_assignment
20:fn reload_layout_repositions_output_via_below
38:fn reload_layout_coord_pin_overrides_chain
56:fn reload_layout_empty_entries_falls_back_to_auto_chain
75:fn reload_layout_safety_net_re_enables_all_disabled
91:fn reload_layout_noop_when_registry_empty
103:fn reload_layout_with_mode_change_safely_logs_only_in_harness
125:fn reload_with_enabled_false_simulates_disable_via_registry
142:fn reload_re_enabling_output_re_adds_to_registry
166:fn reload_safety_net_re_enables_all_disabled_via_harness
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_hotplug_tests/windows.rs</code> &mdash; 269 lines</summary>

```rust
2:fn window_on_removed_output_stays_at_logical_position
19:fn window_on_remaining_output_undisturbed_by_sibling_removal
35:fn window_straddling_two_outputs_loses_one_on_removal
51:fn reconfigure_geometry_keeps_window_position_updates_overlap
67:fn cyclic_add_remove_add_does_not_leak_entered_outputs
85:fn multiple_windows_distributed_across_outputs_snapshot
105:fn multi_workspace_window_survives_output_remove
127:fn reload_position_change_to_below_window_loses_overlap
149:fn reload_position_change_brings_output_under_window
171:fn disable_then_re_enable_output_window_overlap_restored
202:fn window_moved_between_workspaces_preserves_overlap
222:fn reconfigure_grow_brings_window_into_output
238:fn safety_net_re_enable_keeps_window_overlap
258:fn entry_with
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_layout.rs</code> &mdash; 301 lines</summary>

```rust
5:pub enum OutputPosition
19:pub struct OutputPlacement
27:pub struct OutputLayout
32:pub struct ResolvedOutput
43:pub struct ConnectedOutput
52:pub struct OutputReloadDiff
57:impl OutputLayout
58:    pub fn from_config_entries
64:    pub fn placement_for<'a>
70:    pub fn resolve
146:fn apply_primary_selection
162:fn resolve_position
198:fn auto_position
208:fn resolve_relative_to_target<F>
242:impl From<&OutputEntry> for OutputPlacement
243:    fn from
260:pub fn parse_output_transform
277:pub fn detect_output_reload_diff
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_layout_tests.rs</code> &mdash; 12 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_layout_tests/config_and_diff.rs</code> &mdash; 265 lines</summary>

```rust
2:fn from_config_entries_builds_layout_in_order
16:fn empty_layout_single_output_matches_legacy_x_offset_behavior
30:fn empty_layout_two_outputs_matches_legacy_x_offset_chain
44:fn empty_layout_three_outputs_matches_legacy_x_offset_chain
60:fn from_config_entries_empty_yields_empty_layout
66:fn parse_output_transform_recognizes_all_known_variants
87:fn parse_output_transform_is_case_insensitive
96:fn parse_output_transform_unknown_falls_back_to_normal
101:fn parse_output_transform_trims_whitespace
106:fn safety_net_force_enables_when_all_disabled
123:fn safety_net_noop_when_at_least_one_enabled
143:fn safety_net_noop_when_resolved_empty
150:fn single_monitor_default_config_remains_enabled_with_normal_transform_scale_one
164:fn single_monitor_disabled_via_config_safety_net_re_enables
183:fn diff_no_change_when_both_none
195:fn diff_no_change_when_identical_modes
206:fn diff_mode_changed_when_dimensions_differ
217:fn diff_mode_changed_when_refresh_differs
228:fn diff_mode_changed_when_one_side_is_none
239:fn diff_enabled_changed_toggles
250:fn diff_enabled_unchanged_when_default_true_both_sides
257:fn diff_combined_mode_and_enabled_change
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_layout_tests/fixtures.rs</code> &mdash; 73 lines</summary>

```rust
1:fn placement
15:fn connected
23:fn expected_output
43:fn config_entry
56:fn config_entry_with_mode
67:fn mode
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_layout_tests/resolution.rs</code> &mdash; 347 lines</summary>

```rust
2:fn empty_layout_two_outputs_chains_horizontally
21:fn coord_position_places_at_exact_xy
44:fn right_of_resolves_to_target_right_edge
65:fn left_of_uses_self_width
94:fn below_stacks_vertically
115:fn above_uses_self_height
144:fn dangling_reference_falls_back_to_auto
165:fn cycle_two_outputs_falls_back_to_auto_for_second
184:fn disabled_output_is_skipped_in_chain
206:fn explicit_primary_overrides_first
222:fn multiple_primary_keeps_first_in_placements
241:fn no_enabled_output_yields_no_primary
254:fn connected_order_preserved_in_result
274:fn placement_for_returns_existing_or_none
284:fn from_config_entry_maps_auto_position
293:fn from_config_entry_maps_coord_position
307:fn from_config_entry_maps_each_relation
340:fn from_config_entry_preserves_primary_and_enabled
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_power.rs</code> &mdash; 205 lines</summary>

```rust
4:pub enum OutputPowerMode
13:pub struct OutputPowerManager
17:impl OutputPowerManager
18:    pub fn new
22:    pub fn mode_for
27:    pub fn set_mode
38:    pub fn forget
42:    pub fn known_count
48:    pub fn projected_on_count
72:    fn default_mode_for_unknown_output_is_on
79:    fn set_mode_off_returns_true_first_time
87:    fn set_mode_same_returns_false
95:    fn set_mode_on_after_off_returns_true
103:    fn forget_removes_mode_and_returns_previous
112:    fn forget_unknown_returns_default_on
119:    fn projected_on_count_no_change_keeps_count
129:    fn projected_on_count_turning_off_one_of_many
139:    fn projected_on_count_turning_off_last_returns_zero
149:    fn projected_on_count_turning_on_already_off
160:    fn safety_net_rejects_last_on_off_via_projected_count
170:    fn safety_net_allows_off_when_other_on_exists
180:    fn safety_net_allows_on_anytime
191:    fn cycle_off_on_off_with_multiple_outputs_consistent
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_registry.rs</code> &mdash; 223 lines</summary>

```rust
6:pub struct OutputId
9:pub struct OutputGeometry
16:impl OutputGeometry
17:    pub fn contains
27:pub struct OutputModeInfo
36:pub struct OutputInfo
47:pub struct OutputRegistration
56:pub struct OutputReconfigure
65:pub struct OutputRegistry
71:impl OutputRegistry
72:    pub fn new
76:    pub fn list
80:    pub fn modes_for_id
84:    pub fn set_modes_by_id
92:    pub fn set_modes_by_name
101:    pub fn first
105:    pub fn primary
112:    pub fn by_id
116:    pub fn by_name
120:    pub fn contains_id
124:    pub fn contains_name
128:    pub fn output_at_point
134:    pub fn select_for_point_with_fallback
138:    pub fn upsert
166:    fn ensure_primary_after_mutation
178:    pub fn remove_by_id
186:    pub fn remove_by_name
194:    pub fn reconfigure_by_id
208:    pub fn reconfigure_by_name
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_registry_tests.rs</code> &mdash; 388 lines</summary>

```rust
7:fn reg
22:fn reconfigure
38:fn register_and_list_outputs
47:fn output_modes_are_stored_by_output_id
66:fn primary_and_first_fallback_work
82:fn lookup_by_id_works
93:fn output_at_point_handles_two_horizontal_outputs
113:fn select_for_point_with_fallback_prefers_point_match
126:fn select_for_point_with_fallback_uses_primary_before_first
171:fn select_for_point_with_fallback_uses_first_when_no_primary_exists
216:fn select_for_point_with_fallback_is_none_when_empty
224:fn empty_registry_is_safe
234:fn remove_by_id_removes_output
245:fn remove_unknown_output_is_safe_noop
254:fn reconfigure_keeps_output_id
263:fn reconfigure_updates_geometry_and_scale
278:fn primary_fallback_works_after_primary_remove
289:fn output_id_is_not_reused_after_remove_and_add
298:fn remove_primary_promotes_first_remaining_to_primary
312:fn remove_non_primary_does_not_change_primary_flag
324:fn remove_only_output_leaves_empty_registry_without_panic
335:fn remove_by_name_promotes_first_remaining_to_primary
348:fn remove_when_no_primary_was_set_still_promotes_first
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/session_lock_tests.rs</code> &mdash; 97 lines</summary>

```rust
4:fn default_phase_is_unlocked
10:fn lock_request_transitions_to_pending_then_locked
19:fn double_lock_request_is_rejected
27:fn confirm_without_pending_is_noop
34:fn unlock_from_locked_returns_to_unlocked_and_clears_surfaces
44:fn unlock_from_pending_returns_to_unlocked
52:fn unlock_from_unlocked_is_noop
60:fn drop_surface_for_unknown_output_is_noop
67:fn full_lifecycle_without_surfaces
77:fn lock_with_targets_via_test_helper_then_record_all_reaches_locked
90:fn lock_then_output_removed_drains_target
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/setup.rs</code> &mdash; 196 lines</summary>

```rust
63:pub
69:pub
128:pub
147:fn xkb_value
164:    fn parses_vconsole_layout_and_options
173:    fn handles_quotes_and_ignores_comments
180:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/setup/dirty_and_construction.rs</code> &mdash; 281 lines</summary>

```rust
1:impl MeridianState
2:    pub fn mark_all_outputs_dirty
25:    pub fn mark_output_dirty
48:    pub fn mark_output_dirty_by_name
72:    pub fn new
233:    fn post_output_state_change
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/setup/layout.rs</code> &mdash; 440 lines</summary>

```rust
1:impl MeridianState
9:    pub fn reapply_output_layout
277:    fn build_and_register_disabled_output
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/setup/output_lifecycle.rs</code> &mdash; 216 lines</summary>

```rust
1:impl MeridianState
2:    pub fn handle_output_added_or_updated
60:    fn reclamp_windows_to_live_outputs
81:    pub fn handle_output_removed
122:    pub fn handle_output_reconfigured
163:    pub fn register_output_info
167:    pub fn output_geometry_for_registry
176:    fn init_wayland_listener
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/setup_tests.rs</code> &mdash; 64 lines</summary>

```rust
6:fn apply_config_overrides_marks_cursor_change_when_cursor_override_differs
26:fn apply_config_overrides_marks_wallpaper_change_and_updates_override
52:fn apply_config_overrides_with_unknown_theme_keeps_current_theme_and_flags_unchanged
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/utils.rs</code> &mdash; 111 lines</summary>

```rust
13:pub
21:pub
25:pub
51:pub
74:pub
90:pub
102:    fn x11_window_id_key_matches_window_list_scheme
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/workspace_output_state.rs</code> &mdash; 411 lines</summary>

```rust
6:pub struct WorkspaceOutputState
11:impl WorkspaceOutputState
12:    pub fn raw_focused_output
16:    pub fn has_stale_focused_output
21:    fn fallback_output_id
28:    pub fn focused_output
34:    pub fn set_focused_output
49:    pub fn active_workspace_for_output
63:    pub fn set_active_workspace_for_output
91:    pub fn sync_outputs_with_workspace_state
166:    fn reg
181:    fn reconfigure_primary
197:    fn single_output_initializes_focused_output
206:    fn sync_creates_active_workspace_mapping_for_outputs
223:    fn unknown_output_fallback_is_safe
237:    fn set_get_active_workspace_per_output
250:    fn invalid_target_for_output_mapping_is_ignored
263:    fn focused_output_mapping_is_used_for_current_workspace_read
280:    fn missing_focused_output_falls_back_to_global_active
292:    fn missing_mapping_falls_back_to_global_active
305:    fn sync_removes_stale_output_mapping
316:    fn removed_focused_output_falls_back_to_primary
333:    fn removed_focused_output_falls_back_to_first_when_no_primary
350:    fn all_outputs_removed_clears_focused_output
362:    fn reconfigure_keeps_focused_output_and_mapping_for_same_id
395:    fn add_new_output_gets_global_active_mapping_and_focus_stays_stable
```

</details>

<details><summary><code>crates/meridian-compositor/src/wallpaper/compose.rs</code> &mdash; 159 lines</summary>

```rust
7:pub
18:pub
67:fn solid_fallback
84:    fn pixel
90:    fn compose_without_image_uses_expected_solid_fallback
100:    fn center_places_single_pixel_in_canvas_center
113:    fn tile_repeats_source_pattern
131:    fn fill_and_fit_modes_preserve_expected_uniform_and_letterbox_behavior
```

</details>

<details><summary><code>crates/meridian-compositor/src/wallpaper/gpu.rs</code> &mdash; 77 lines</summary>

```rust
19:pub struct WallpaperGpuCache
25:impl WallpaperGpuCache
26:    fn needs_update
35:    pub fn update
67:    pub fn render_element
```

</details>

<details><summary><code>crates/meridian-compositor/src/wallpaper/manager.rs</code> &mdash; 82 lines</summary>

```rust
13:pub struct WallpaperManager
19:impl WallpaperManager
20:    pub fn new
28:    pub fn apply_theme
42:    fn ensure_loaded
65:    pub fn compose_for_size
70:    pub fn source_key
78:impl Default for WallpaperManager
79:    fn default
```

</details>

<details><summary><code>crates/meridian-compositor/src/wallpaper/mod.rs</code> &mdash; 6 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-compositor/src/workspace.rs</code> &mdash; 324 lines</summary>

```rust
14:pub struct WorkspaceManager<E: SpaceElement = Window>
20:    fn default
26:    pub fn new
33:    pub fn count
37:    pub fn active_space
41:    pub fn active_space_mut
45:    pub fn space_at
49:    pub fn space_at_mut
57:    pub fn find_element_workspace<F>
76:    pub fn reclamp_offscreen_windows
108:    fn can_target_workspace
113:    pub fn try_switch
123:    pub fn move_window_to
136:    pub fn remap_outputs
156:    struct MockWindow
160:    impl IsAlive for MockWindow
161:        fn alive
166:    impl SpaceElement for MockWindow
167:        fn bbox
170:        fn is_in_input_region
173:        fn set_activate
174:        fn output_enter
175:        fn output_leave
178:    fn manager
183:    fn try_switch_ignores_invalid_target
191:    fn try_switch_ignores_current_workspace
199:    fn try_switch_updates_active_workspace_on_valid_target
207:    fn move_guards_share_same_target_validation
217:    fn find_element_workspace_finds_window_on_non_active_workspace
229:    fn find_element_workspace_is_none_for_absent_or_empty
238:    fn move_window_to_relocates_and_leaves_no_duplicate
256:    fn move_window_to_rejects_active_and_out_of_range
268:    fn output
273:    fn reclamp_moves_window_stranded_off_all_outputs
288:    fn reclamp_leaves_onscreen_windows_untouched
301:    fn reclamp_runs_across_all_workspaces
314:    fn reclamp_without_outputs_is_noop
```

</details>

### `meridian-config`

<details><summary><code>crates/meridian-config/src/config.rs</code> &mdash; 286 lines</summary>

```rust
17:pub struct GeneralConfig
23:impl Default for GeneralConfig
24:    fn default
33:pub struct CursorConfig
38:impl Default for CursorConfig
39:    fn default
52:pub struct WallpaperConfig
57:impl Default for WallpaperConfig
58:    fn default
67:pub struct PinnedAppConfig
74:pub struct PanelConfig
81:pub struct WallpaperEntry
90:pub struct MeridianConfig
99:impl MeridianConfig
100:    pub fn load
105:    pub fn reload
110:    pub fn reload_from_path
122:    fn load_from
167:    pub fn wallpaper_override
175:    fn load_or_default_from_path
197:fn config_directory
204:struct PinnedAppToml
213:struct PanelToml
219:struct MeridianToml
230:struct GeneralToml
236:impl Default for GeneralToml
237:    fn default
247:struct CursorToml
252:impl Default for CursorToml
253:    fn default
263:struct WallpaperToml
268:impl Default for WallpaperToml
269:    fn default
```

</details>

<details><summary><code>crates/meridian-config/src/config/mutation.rs</code> &mdash; 324 lines</summary>

```rust
1:impl MeridianConfig
4:    pub fn save_theme
67:    pub fn save_wallpaper
105:    pub fn save_cursor
131:    pub fn save_idle_timeout
152:    pub fn save_pinned_apps
190:    pub fn save_primary_output
219:    pub fn save_output_mode
256:    pub fn save_output_scale
272:    pub fn save_output_transform
288:    pub fn scan_wallpaper_dirs
```

</details>

<details><summary><code>crates/meridian-config/src/config/output_toml.rs</code> &mdash; 349 lines</summary>

```rust
1:fn strip_toml_section
26:fn set_cursor_in_toml
37:fn set_output_mode_in_toml
102:fn set_primary_output_in_toml
165:fn output_section_name
180:fn is_mode_key_line
188:fn is_primary_key_line
196:fn push_output_mode_line
208:fn push_primary_line
220:fn write_output_key
241:fn remove_output_key
255:fn is_output_key_line
266:fn set_output_key_in_toml
327:fn remove_output_key_in_toml
```

</details>

<details><summary><code>crates/meridian-config/src/config/wallpapers.rs</code> &mdash; 166 lines</summary>

```rust
12:fn collect_images_by_dir
44:fn wallpaper_entry_display_name
62:fn wallpaper_dir_display_name
75:fn has_general_section
81:fn find_theme_line
88:fn find_general_key_line
110:fn set_idle_timeout_in_toml
```

</details>

<details><summary><code>crates/meridian-config/src/config_tests.rs</code> &mdash; 8 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-config/src/config_tests/output_updates.rs</code> &mdash; 277 lines</summary>

```rust
2:fn outputs_section_parses_two_outputs_with_relative_position
29:fn outputs_position_table_variants_parse
68:fn outputs_position_coord_inline_table_parses
86:fn outputs_position_string_auto_parses
104:fn outputs_position_string_other_returns_error
136:fn outputs_position_multiple_relations_returns_error
150:fn outputs_position_xy_mixed_with_relation_returns_error
164:fn outputs_position_only_x_returns_error
178:fn outputs_position_empty_table_means_auto
196:fn outputs_defaults_when_only_name_set
216:fn outputs_mode_table_parses
255:fn outputs_transform_passes_through_string_unvalidated
```

</details>

<details><summary><code>crates/meridian-config/src/config_tests/parsing.rs</code> &mdash; 323 lines</summary>

```rust
2:fn panel_pinned_apps_parse_from_toml
28:fn panel_section_missing_gives_empty_pinned
36:fn panel_pinned_unknown_field_returns_error
46:fn missing_file_uses_defaults
55:fn valid_toml_parses_general_cursor_and_wallpaper
86:fn invalid_toml_falls_back_to_defaults
96:fn cursor_and_wallpaper_modes_parse
122:fn reload_from_path_with_valid_file_updates_all_sections
154:fn reload_from_path_with_invalid_file_returns_error_and_preserves_old_config
174:fn set_cursor_in_toml_replaces_existing_section_and_round_trips
205:fn set_cursor_in_toml_appends_when_absent
217:fn set_idle_timeout_replaces_existing_key_and_round_trips
231:fn set_idle_timeout_inserts_into_existing_general_section
242:fn set_idle_timeout_appends_general_when_absent
254:fn set_idle_timeout_none_removes_the_key
267:fn set_idle_timeout_none_on_missing_key_is_noop
273:fn reload_from_path_with_missing_file_resets_to_defaults
289:fn keybinds_section_remains_supported
304:fn reload_from_path_with_invalid_keybind_keeps_previous_config
```

</details>

<details><summary><code>crates/meridian-config/src/config_tests/wallpaper_and_idle.rs</code> &mdash; 257 lines</summary>

```rust
2:fn outputs_section_missing_keeps_empty_vec
17:fn outputs_unknown_field_returns_error
31:fn reload_with_outputs_replaces_previous_set
63:fn outputs_section_preserved_with_other_sections
114:fn set_primary_output_updates_existing_output_sections
141:fn set_primary_output_appends_missing_output_section
156:fn set_output_mode_updates_existing_output_section
175:fn set_output_mode_appends_missing_output_section
186:fn set_output_key_replaces_existing_scale_in_section
200:fn set_output_key_inserts_transform_when_absent_and_round_trips
214:fn set_output_key_appends_section_when_output_absent
221:fn remove_output_key_drops_transform_only_for_target
231:fn output_by_name<'a>
239:fn unique_test_path
252:fn write
```

</details>

<details><summary><code>crates/meridian-config/src/keybind/defaults.rs</code> &mdash; 37 lines</summary>

```rust
3:pub
```

</details>

<details><summary><code>crates/meridian-config/src/keybind/mod.rs</code> &mdash; 185 lines</summary>

```rust
11:    pub struct Modifiers: u8
20:pub struct Keybind
25:impl Keybind
26:    pub fn new
32:pub enum Action
46:pub enum SplitDir
52:pub struct KeybindConfig
56:impl KeybindConfig
57:    pub fn bindings
61:    pub fn find_action
68:    pub fn from_map
83:impl Default for KeybindConfig
84:    fn default
100:pub struct KeybindToml
111:    fn valid_keybinds_are_parsed
120:    fn invalid_keybind_returns_controlled_error
128:    fn defaults_include_workspace_switch_1_to_9
147:    fn defaults_include_move_to_workspace_1_to_9
167:    fn reload_config_action_is_bindable
178:    fn defaults_do_not_include_reload_config_binding
```

</details>

<details><summary><code>crates/meridian-config/src/keybind/parse.rs</code> &mdash; 182 lines</summary>

```rust
3:pub
27:fn keysym_from_name
123:pub
173:fn parse_workspace_number
```

</details>

<details><summary><code>crates/meridian-config/src/lib.rs</code> &mdash; 15 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-config/src/output.rs</code> &mdash; 206 lines</summary>

```rust
4:pub enum OutputPositionConfig
18:pub struct OutputModeConfig
25:pub struct OutputEntry
35:impl OutputEntry
36:    pub fn defaults_for
51:enum OutputPositionToml
58:struct OutputPositionTableToml
69:impl Default for OutputPositionToml
70:    fn default
77:struct OutputModeToml
85:pub
96:impl Default for OutputToml
97:    fn default
109:fn default_enabled
113:fn default_scale
117:impl OutputPositionToml
118:    fn into_config
190:impl OutputToml
191:    pub
```

</details>

<details><summary><code>crates/meridian-config/src/theme/manager.rs</code> &mdash; 366 lines</summary>

```rust
12:pub struct Theme
18:impl Theme
19:    fn builtin_default
27:    fn load_from_dir
41:    pub fn css_path
49:    pub fn asset_path
57:    pub fn wallpaper_path
71:type ThemeChangedCallback = Box<dyn Fn
73:pub struct ThemeManager
80:impl ThemeManager
81:    pub fn new
94:    fn new_with_dirs_for_tests
105:    pub fn current
109:    pub fn current_mut
113:    pub fn themes_dir
117:    pub fn theme_dirs
121:    pub fn set_theme
130:    pub fn reload
135:    pub fn available_themes
139:    pub fn register_observer
143:    fn notify_observers
150:impl Default for ThemeManager
151:    fn default
156:impl fmt::Debug for ThemeManager
157:    fn fmt
167:fn expand_tilde
180:fn user_theme_directory
188:fn theme_directories
219:fn dev_theme_directory
223:fn push_unique_path
230:fn load_named_theme
240:fn available_theme_names
262:fn load_or_default
288:    struct TempDir
292:    impl TempDir
293:        fn new
307:        fn path
312:    impl Drop for TempDir
313:        fn drop
318:    fn write_theme
334:    fn available_themes_scans_all_configured_dirs
352:    fn set_theme_prefers_earlier_dirs
```

</details>

<details><summary><code>crates/meridian-config/src/theme/mod.rs</code> &mdash; 8 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-config/src/theme/types/color.rs</code> &mdash; 6 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-config/src/theme/types/config.rs</code> &mdash; 478 lines</summary>

```rust
9:pub struct ThemeColors
23:impl Default for ThemeColors
28:    fn default
48:pub struct Decorations
85:pub enum ThemeSurface
94:pub struct SurfaceTreatment
102:impl Default for Decorations
109:    fn default
136:impl Decorations
137:    pub fn surface_radius
149:    pub fn surface_treatment
185:fn alpha_byte
191:pub struct Fonts
195:impl Default for Fonts
198:    fn default
205:impl Fonts
209:    pub fn ui_family
224:pub struct Icons
228:impl Default for Icons
229:    fn default
238:pub struct Cursor
243:impl Default for Cursor
244:    fn default
256:pub enum WallpaperMode
264:impl fmt::Display for WallpaperMode
265:    fn fmt
276:pub struct Wallpaper
284:pub struct ThemeConfig
293:impl ThemeConfig
294:    pub fn glass_tint_color
300:    pub fn appearance_is_light
312:    fn test_theme_colors_default_is_dark_palette
329:    fn test_decorations_default_central_glass
348:    fn test_glass_tint_color_defaults_none_and_parses
367:    fn test_theme_config_partial_toml_fills_new_defaults
398:    fn surface_treatment_uses_theme_radius_and_glass_alpha
432:    fn surface_treatment_non_glass_is_opaque_without_blur
449:    fn theme_config_appearance_tracks_background_luminance
458:    fn test_fonts_default_uses_inter
464:    fn ui_family_strips_trailing_size
473:    fn test_cursor_default_uses_installed_theme
```

</details>

<details><summary><code>crates/meridian-config/src/theme/types/error.rs</code> &mdash; 32 lines</summary>

```rust
4:pub enum ThemeError
10:impl fmt::Display for ThemeError
11:    fn fmt
20:impl std::error::Error for ThemeError
22:impl From<std::io::Error> for ThemeError
23:    fn from
28:impl From<toml::de::Error> for ThemeError
29:    fn from
```

</details>

<details><summary><code>crates/meridian-config/src/theme/types/mod.rs</code> &mdash; 10 lines</summary>

```rust
```

</details>

### `meridian-freetype`

<details><summary><code>crates/meridian-freetype/src/lib.rs</code> &mdash; 242 lines</summary>

```rust
9:type FtError = c_int;
10:type FtLibrary = *mut c_void;
11:type FtFace = *mut FtFaceRec;
12:type FtGlyphSlot = *mut FtGlyphSlotRec;
15:struct FtGeneric
22:struct FtVector
28:struct FtBbox
36:struct FtBitmap
48:struct FtGlyphMetrics
60:struct FtGlyphSlotRec
77:struct FtFaceRec
104:    fn FT_Init_FreeType
105:    fn FT_Done_FreeType
106:    fn FT_New_Memory_Face
113:    fn FT_Done_Face
114:    fn FT_Set_Pixel_Sizes
115:    fn FT_Load_Char
118:pub struct Glyph
127:pub struct Font
135:impl Font
136:    pub fn from_static_bytes
157:    fn set_size
162:    pub fn rasterize
204:    pub fn measure_text
218:impl Drop for Font
219:    fn drop
232:    fn freetype_rasterizes_embedded_font
```

</details>

### `meridian-ipc`

<details><summary><code>crates/meridian-ipc/src/lib.rs</code> &mdash; 362 lines</summary>

```rust
9:pub struct WindowSnapshotEntry
19:fn default_output_scale_millis
24:pub struct OutputModeState
36:pub struct OutputWorkspaceState
60:impl Default for OutputWorkspaceState
61:    fn default
81:pub struct OutputWorkspaceSnapshot
88:pub enum ScreenshotKind
94:pub enum ScreenshotRequestOrigin
102:pub struct ScreenshotRequestMetadata
118:pub struct ScreenshotRegion
126:pub struct ScreenshotBridgeRequest
136:impl ScreenshotBridgeRequest
137:    pub fn validate
167:pub struct ScreenshotBridgeResponse
174:pub enum ScreenshotBridgeError
184:pub enum ScreenshotBridgeResult
191:pub enum ScreenshotBridgeMessage
203:pub enum ShellEvent
278:pub enum ShellCommand
319:pub fn socket_path
329:pub fn encode_command
333:pub fn encode_event
337:pub fn decode_command
341:pub fn decode_event
345:pub fn encode_screenshot_bridge_message
349:pub fn decode_screenshot_bridge_message
353:fn encode_json_line<T: Serialize>
```

</details>

<details><summary><code>crates/meridian-ipc/src/lib_tests.rs</code> &mdash; 431 lines</summary>

```rust
11:fn window_snapshot_entry_contains_workspace_id_and_title
26:fn window_snapshot_event_roundtrip_supports_multiple_workspaces
60:fn output_workspace_changed_event_roundtrip_supports_optional_name
74:fn output_workspace_snapshot_event_roundtrip_supports_two_outputs
134:fn legacy_workspace_changed_roundtrip_remains_stable
142:fn window_focus_cleared_event_roundtrip_is_supported
150:fn desktop_context_menu_event_roundtrip_is_supported
158:fn output_workspace_snapshot_allows_missing_focused_output
176:fn output_workspace_snapshot_decodes_legacy_output_without_display_details
194:fn window_snapshot_entry_missing_minimized_decodes_to_false
201:fn launch_app_command_roundtrip_uses_argv
219:fn launch_app_command_accepts_legacy_command_field
233:fn quit_command_roundtrip_is_supported
241:fn screenshot_bridge_request_supports_full_output_mode
261:fn screenshot_bridge_request_rejects_empty_request_id
280:fn screenshot_bridge_request_accepts_region_with_nonzero_size
299:fn screenshot_bridge_request_rejects_region_with_zero_dimension
323:fn screenshot_bridge_request_roundtrip_works
348:fn screenshot_bridge_response_roundtrip_with_error_works
366:fn screenshot_bridge_response_roundtrip_with_success_works
384:fn screenshot_bridge_request_metadata_roundtrip_works
409:fn capture_window_thumbnail_command_roundtrip_is_supported
421:fn window_thumbnail_event_roundtrip_is_supported
```

</details>

### `meridian-lock`

<details><summary><code>crates/meridian-lock/src/auth.rs</code> &mdash; 217 lines</summary>

```rust
35:struct ConvData
40:impl Drop for ConvData
41:    fn drop
96:struct PamGuard
101:impl Drop for PamGuard
102:    fn drop
112:pub fn authenticate
181:        fn auth_userokay
190:    pub fn authenticate
```

</details>

<details><summary><code>crates/meridian-lock/src/main.rs</code> &mdash; 454 lines</summary>

```rust
26:struct LockStyle
40:impl LockStyle
41:    fn from_theme
67:    fn load
86:struct LockSurface
100:struct AppState
121:enum LockStatus
127:impl AppState
128:    fn mark_all_dirty
137:impl Dispatch<wl_registry::WlRegistry,
138:    fn event
192:impl Dispatch<wl_output::WlOutput,
193:    fn event
206:impl Dispatch<wl_seat::WlSeat,
207:    fn event
226:impl Dispatch<wl_keyboard::WlKeyboard,
227:    fn event
293:fn handle_key
362:impl Dispatch<ext_session_lock_manager_v1::ExtSessionLockManagerV1,
363:    fn event
374:impl Dispatch<ext_session_lock_v1::ExtSessionLockV1,
375:    fn event
399:impl Dispatch<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
400:    fn event
```

</details>

<details><summary><code>crates/meridian-lock/src/main/render.rs</code> &mdash; 210 lines</summary>

```rust
1:fn render_frame
118:fn create_lock_surface
140:fn render_surface
190:fn get_username
```

</details>

<details><summary><code>crates/meridian-lock/src/main/render_helpers.rs</code> &mdash; 233 lines</summary>

```rust
1:fn create_anonymous_shm
5:            fn shm_mkstemp
30:fn create_shm_buffer
78:fn color
86:fn fill_rect
117:fn fill_circle
140:fn draw_lock_icon
197:struct TextMetrics
203:fn measure_text
213:fn draw_text
228:fn draw_text_centered
```

</details>

<details><summary><code>crates/meridian-lock/src/main/runtime.rs</code> &mdash; 125 lines</summary>

```rust
1:fn main
```

</details>

<details><summary><code>crates/meridian-lock/src/main_tests.rs</code> &mdash; 137 lines</summary>

```rust
5:fn us_keymap_state
13:fn test_state
44:fn color_unpacks_rgba_channels
66:fn typing_appends_characters
75:fn backspace_removes_one_ascii_char
83:fn backspace_removes_one_full_utf8_char
95:fn backspace_on_empty_password_is_noop
102:fn escape_clears_password_and_resets_status
112:fn auth_in_progress_blocks_input
123:fn return_with_empty_password_does_not_start_auth
131:fn typing_resets_failed_status_to_idle
```

</details>

### `meridian-login`

<details><summary><code>crates/meridian-login/src/auth.rs</code> &mdash; 497 lines</summary>

```rust
69:pub enum AuthResult
85:pub enum AuthBackend
90:impl AuthBackend
91:    fn pam_service
104:pub struct AuthDriver
109:impl AuthDriver
112:    pub fn close
120:impl Drop for AuthDriver
121:    fn drop
133:pub fn start_auth_session
156:struct ConvData
161:impl Drop for ConvData
162:    fn drop
257:fn drain_pam_env
293:struct PamGuard
298:impl Drop for PamGuard
299:    fn drop
310:fn run_pam_session
450:    fn empty_username_returns_failed_quickly
471:    fn auth_backend_selects_expected_pam_service
480:    fn drain_pam_env_on_null_handle_is_empty
```

</details>

<details><summary><code>crates/meridian-login/src/auth/openbsd_backend.rs</code> &mdash; 153 lines</summary>

```rust
12:    fn auth_userokay
21:pub enum AuthResult
28:pub enum AuthBackend
33:pub struct AuthDriver
38:impl AuthDriver
39:    pub fn close
47:impl Drop for AuthDriver
48:    fn drop
56:pub fn start_auth_session
94:fn authenticate_password
127:    fn empty_username_returns_failed
141:    fn smartcard_reports_explicit_configuration_error
```

</details>

<details><summary><code>crates/meridian-login/src/input.rs</code> &mdash; 534 lines</summary>

```rust
21:pub enum KeyAction
37:pub struct KeyboardStatus
42:impl Default for KeyboardStatus
43:    fn default
52:pub enum PointerAction
57:struct AxisRange
62:impl AxisRange
63:    fn normalize
69:pub struct PointerDevice
76:pub struct PointerState
83:impl PointerState
84:    pub fn new
95:    fn move_relative
100:    fn move_absolute_x
104:    fn move_absolute_y
110:pub struct Keyboard
118:impl Keyboard
119:    pub fn new
142:    pub fn status
155:    fn process
204:fn is_keyboard_keyset
223:fn keypad_digit
240:pub fn open_keyboards
300:pub fn open_pointers
384:pub fn poll_keyboards
414:pub fn poll_pointers
469:fn read_system_layout
488:    fn keyboard_constructs_with_default_layout
494:    fn read_system_layout_returns_some_or_none_without_panic
500:    fn keyboard_status_exposes_layout_and_caps_lock
508:    fn process_filters_pure_control_chars
518:    fn process_maps_keypad_digits_without_numlock
529:    fn process_maps_keypad_enter_to_submit
```

</details>

<details><summary><code>crates/meridian-login/src/main.rs</code> &mdash; 304 lines</summary>

```rust
83:type Rect =
84:type PowerButtonRects =
91:fn card_radius
97:fn control_radius
128:struct Card
130:impl AsFd for Card
131:    fn as_fd
135:impl DrmDevice for Card
136:impl ControlDevice for Card
139:fn card_drives_a_display
163:fn open_display_card
204:struct LoginUiState
231:enum InputPhase
244:enum Field
251:enum ControlFlow
260:enum ClickTarget
268:enum PowerAction
274:struct PendingPowerAction
279:impl PowerAction
280:    fn control_flow
288:impl PendingPowerAction
289:    fn is_active
```

</details>

<details><summary><code>crates/meridian-login/src/main/animation.rs</code> &mdash; 266 lines</summary>

```rust
1:struct AnimFrame
6:fn compute_anim_frame
18:fn anim_frame_is_steady
22:fn ramp_f32
34:fn run_animation
```

</details>

<details><summary><code>crates/meridian-login/src/main/controls.rs</code> &mdash; 378 lines</summary>

```rust
1:fn caret_x
9:fn draw_submit_button
63:fn draw_brand_mark
122:fn draw_user_icon
137:fn draw_lock_icon
152:fn draw_yubikey_icon
264:fn draw_input_box
306:fn draw_caret
323:fn draw_pointer_cursor
356:fn card_rect
364:fn rounded_rect_path
```

</details>

<details><summary><code>crates/meridian-login/src/main/geometry_and_buttons.rs</code> &mdash; 298 lines</summary>

```rust
1:fn click_target_at
43:fn login_button_rect
53:fn smartcard_pin_rect
65:fn power_button_rects
81:fn run_power_action
95:fn color_with_alpha
104:fn mix_color
114:fn alpha_byte
118:fn theme_color
122:fn modal_fill_alpha
129:fn modal_frame_alpha
136:fn metro_surface
140:fn metro_background
148:fn metro_accent
152:fn metro_text
156:fn metro_text_dim
160:fn metro_border
164:fn metro_error
168:fn metro_success
172:fn draw_soft_card_shadow
204:fn draw_card_stroke
215:fn draw_login_button
258:fn draw_power_buttons
296:fn point_in_rect
```

</details>

<details><summary><code>crates/meridian-login/src/main/handover_ipc.rs</code> &mdash; 126 lines</summary>

```rust
1:fn bootsplash_handover
5:fn bootsplash_exit
9:fn bootsplash_socket_path
13:fn send_command
42:enum IpcEvent
62:fn spawn_login_ipc_server
103:fn handle_login_ipc_client
```

</details>

<details><summary><code>crates/meridian-login/src/main/runtime.rs</code> &mdash; 329 lines</summary>

```rust
1:fn main
```

</details>

<details><summary><code>crates/meridian-login/src/main/state.rs</code> &mdash; 357 lines</summary>

```rust
1:impl LoginUiState
2:    fn apply
56:    fn start_auth
74:    fn poll_auth
90:    fn tick
98:    fn confirm_power_action
113:    fn clear_expired_power_confirmation
125:    fn pending_power_action
131:    fn reject
141:    fn shake_offset
156:    fn hint
192:    fn smartcard_login_ready
196:    fn auth_username
209:    fn update_security_key_state
244:fn yubikey_present
249:fn yubikey_present_in_sysfs
267:fn yubikey_present_in_hidraw_sysfs
280:fn hid_id_vendor_from_uevent
287:fn hid_name_from_uevent
293:fn is_yubico_vendor_id
301:fn is_yubikey_name
306:fn smartcard_user_from_authfile
314:fn keyboard_layout_label
334:fn light_appearance
338:fn load_login_theme
355:fn login_theme
```

</details>

<details><summary><code>crates/meridian-login/src/main/ui.rs</code> &mdash; 259 lines</summary>

```rust
1:fn draw_card
41:fn draw_login_ui
```

</details>

<details><summary><code>crates/meridian-login/src/main_tests.rs</code> &mdash; 506 lines</summary>

```rust
3:fn smartcard_ready_state
13:fn anim_frame_at_t0_matches_settle_state
20:fn anim_frame_at_ui_fade_end_is_full
28:fn anim_frame_reports_steady_after_intro
35:fn card_rect_clamped_dimensions
42:fn yubikey_detector_matches_yubico_vendor_id
57:fn yubikey_detector_accepts_zero_padded_hid_vendor_id
64:fn yubikey_detector_matches_yubikey_product_name
80:fn hidraw_detector_matches_yubico_hid_id
99:fn hidraw_detector_matches_yubikey_hid_name
118:fn yubikey_detector_ignores_other_vendor_ids
133:fn smartcard_user_reads_first_mapping_user
151:fn power_buttons_are_click_targets
181:fn login_button_is_submit_target
197:fn empty_placeholder_keeps_caret_at_text_start
203:fn smartcard_mode_does_not_focus_username_field
211:fn smartcard_mode_focuses_short_pin_field
227:fn manual_mode_focuses_username_and_password_fields
266:fn power_action_requires_second_matching_click
277:fn power_confirmation_switches_and_expires
296:fn rounded_rect_path_does_not_panic_on_small_inputs
301:fn insert_appends_to_pin_field
317:fn backspace_removes_last_char_from_pin_field
329:fn cycle_focus_keeps_pin_field
338:fn manual_mode_edits_username_and_password
355:fn manual_mode_cycles_between_username_and_password
365:fn submit_and_cancel_return_their_control_flow
372:fn reject_keeps_username_and_resets_password_focus
387:fn shake_offset_is_zero_outside_failed_state
393:fn shake_offset_nonzero_inside_failed_window
415:fn hint_changes_with_phase
425:fn hint_warns_when_caps_lock_is_active
437:fn hint_mentions_yubikey_when_present
449:fn hint_allows_password_login_with_unregistered_yubikey
461:fn hint_prompts_touch_while_authenticating_with_yubikey
468:fn poll_auth_returns_none_when_no_thread_running
474:fn start_auth_with_empty_username_returns_failed_quickly
482:fn auth_username_uses_manual_username_without_yubikey
491:fn auth_username_uses_smartcard_mapping_when_ready
500:fn insert_respects_max_field_len
```

</details>

<details><summary><code>crates/meridian-login/src/session.rs</code> &mdash; 304 lines</summary>

```rust
21:pub enum SessionError
32:impl std::fmt::Display for SessionError
33:    fn fmt
51:impl std::error::Error for SessionError
62:pub fn launch_compositor_for
228:fn session_path
233:fn session_path
238:fn runtime_dir_path
243:fn runtime_dir_path
249:fn ensure_runtime_dir
279:    fn unknown_user_yields_user_not_found
285:    fn runtime_dir_path_format
298:    fn openbsd_session_path_can_find_x11_binaries
```

</details>

<details><summary><code>crates/meridian-login/src/visual.rs</code> &mdash; 224 lines</summary>

```rust
13:pub
17:impl LoginBackdrop
18:    pub
30:    pub
39:fn blur_cached_wallpaper
76:fn draw_cover
97:fn draw_tint
124:fn draw_compass_guides
191:fn stroke
212:    fn backdrop_renders_and_copies_typical_output
220:    fn backdrop_rejects_wrong_target_size
```

</details>

### `meridian-polkit`

<details><summary><code>crates/meridian-polkit/src/auth.rs</code> &mdash; 183 lines</summary>

```rust
43:fn find_helper
53:pub fn authenticate_via_helper
```

</details>

<details><summary><code>crates/meridian-polkit/src/dbus.rs</code> &mdash; 313 lines</summary>

```rust
29:pub struct Identity
35:pub struct AuthRequest
51:pub enum Outcome
63:pub enum DbusEvent
69:struct AgentService
77:impl AgentService
81:    async fn begin_authentication
144:    async fn cancel_authentication
159:trait Authority
160:    fn register_authentication_agent
167:    fn unregister_authentication_agent
174:fn unix_session_subject
183:fn parse_identities
205:pub fn spawn
229:async fn run
273:    fn owned_u32
278:    fn unix_session_subject_carries_session_id
288:    fn parse_identities_keeps_unix_user_and_resolves_name
301:    fn parse_identities_skips_non_unix_user
309:    fn parse_identities_skips_entry_without_uid
```

</details>

<details><summary><code>crates/meridian-polkit/src/main.rs</code> &mdash; 154 lines</summary>

```rust
24:fn install_panic_logger
59:fn chrono_now
68:fn main
```

</details>

<details><summary><code>crates/meridian-polkit/src/ui.rs</code> &mdash; 392 lines</summary>

```rust
19:pub enum Status
25:pub struct View<'a>
35:pub fn render
193:fn u32_from_color
197:pub fn fill_rect
230:fn fill_circle
247:fn rgba_to_color
257:pub struct TextMetrics
262:pub fn measure_text
271:pub fn draw_text
286:pub fn draw_text_centered
300:fn wrap_text
336:    fn u32_from_color_packs_argb_with_explicit_alpha
344:    fn rgba_to_color_unpacks_channels
352:    fn color_helpers_roundtrip
359:    fn wrap_text_short_text_stays_one_line
365:    fn wrap_text_empty_yields_single_empty_line
371:    fn wrap_text_wraps_when_too_narrow
378:    fn wrap_text_truncates_with_ellipsis_at_max_lines
```

</details>

<details><summary><code>crates/meridian-polkit/src/wayland.rs</code> &mdash; 125 lines</summary>

```rust
33:fn create_anonymous_shm
37:            fn shm_mkstemp
67:pub struct PamResult
72:pub struct ActiveAuth
85:pub struct AppState
107:struct PopupSurface
```

</details>

<details><summary><code>crates/meridian-polkit/src/wayland/dispatch.rs</code> &mdash; 202 lines</summary>

```rust
1:impl Dispatch<wl_registry::WlRegistry,
2:    fn event
37:impl Dispatch<wl_seat::WlSeat,
38:    fn event
57:impl Dispatch<wl_keyboard::WlKeyboard,
58:    fn event
126:impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
127:    fn event
174:        impl Dispatch<$iface,
175:            fn event
195:pub fn connect
```

</details>

<details><summary><code>crates/meridian-polkit/src/wayland/state.rs</code> &mdash; 328 lines</summary>

```rust
1:impl AppState
2:    pub fn new
18:    pub fn on_auth_request
55:    fn reload_theme
72:    pub fn on_cancel_from_polkit
81:    pub fn on_pam_result
106:    fn finish
115:    fn ensure_popup
153:    fn destroy_popup
167:    pub fn draw
267:    fn handle_key
```

</details>

### `meridian-portal`

<details><summary><code>crates/meridian-portal/src/access.rs</code> &mdash; 44 lines</summary>

```rust
6:type Asv = HashMap<String, OwnedValue>;
19:pub struct AccessImpl;
22:impl AccessImpl
24:    fn version
29:    async fn access_dialog
```

</details>

<details><summary><code>crates/meridian-portal/src/file_chooser.rs</code> &mdash; 221 lines</summary>

```rust
6:type Asv = HashMap<String, OwnedValue>;
10:pub struct FileChooserImpl;
13:impl FileChooserImpl
15:    fn version
19:    async fn open_file
67:    async fn save_file
103:    async fn save_files
135:fn str_asv
144:fn uris_asv
157:fn bool_opt
163:fn str_opt<'a>
167:fn path_to_uri
175:fn file_picker_path
179:fn percent_encode_path
192:fn forward_env
207:    fn path_to_uri_preserves_existing_file_uri
215:    fn path_to_uri_percent_encodes_path_bytes
```

</details>

<details><summary><code>crates/meridian-portal/src/lib.rs</code> &mdash; 65 lines</summary>

```rust
12:pub async fn run
```

</details>

<details><summary><code>crates/meridian-portal/src/main.rs</code> &mdash; 9 lines</summary>

```rust
2:async fn main
```

</details>

<details><summary><code>crates/meridian-portal/src/screenshot.rs</code> &mdash; 260 lines</summary>

```rust
18:type Asv = HashMap<String, OwnedValue>;
25:pub struct ScreenshotImpl;
28:impl ScreenshotImpl
30:    fn version
34:    async fn screenshot
88:    async fn pick_color
102:enum ScreenshotPortalError
110:impl std::fmt::Display for ScreenshotPortalError
111:    fn fmt
122:fn run_request
184:fn uri_asv
192:fn path_to_file_uri
200:fn percent_encode_path
215:fn bool_opt
221:fn next_request_id
237:    fn file_uri_preserves_existing_scheme
245:    fn file_uri_percent_encodes_path_bytes
253:    fn request_ids_are_unique_per_call
```

</details>

<details><summary><code>crates/meridian-portal/src/settings.rs</code> &mdash; 104 lines</summary>

```rust
17:type Asv = HashMap<String, OwnedValue>;
22:pub struct SettingsImpl;
27:impl SettingsImpl
29:    pub
43:    fn color_scheme_value
47:    fn namespace_requested
59:impl SettingsImpl
61:    fn version
66:    fn read_all
80:    fn read_one
91:    fn read
98:    pub
```

</details>

### `meridian-shell`

<details><summary><code>crates/meridian-shell/src/app_view.rs</code> &mdash; 365 lines</summary>

```rust
83:fn cp_settings_btn_x
87:fn cp_hdr_icon_y
91:fn cp_footer_y
95:fn cp_bento_tile_x
101:pub
125:pub
163:pub
170:pub
189:pub
204:pub
228:pub
255:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/app_view/header_and_grid.rs</code> &mdash; 318 lines</summary>

```rust
1:fn draw_header
54:fn draw_bento_strip
139:fn draw_app_grid
232:fn draw_search_results
```

</details>

<details><summary><code>crates/meridian-shell/src/app_view/rows_and_helpers.rs</code> &mdash; 381 lines</summary>

```rust
1:fn draw_app_row_content
31:fn draw_power_footer
129:fn draw_settings_symbol
161:fn arc_seg
183:fn draw_power_symbol
283:fn draw_scrollbar
323:fn section_label
334:fn divider
347:fn divider_col
351:fn fill_rect
357:fn fill_round_rect
363:fn with_alpha
367:fn blit_rgba_to_argb
379:fn to_tiny_skia_color
```

</details>

<details><summary><code>crates/meridian-shell/src/app_view_tests.rs</code> &mdash; 74 lines</summary>

```rust
4:fn blit_rgba_to_argb_swaps_red_and_blue
12:fn blit_twice_roundtrips
22:fn hit_bento_tile_correct_columns
38:fn hit_app_row_grid_mode
53:fn hit_app_row_search_mode
60:fn hit_footer_power_btn_range
69:fn hit_header_settings_btn
```

</details>

<details><summary><code>crates/meridian-shell/src/audio/mixer.rs</code> &mdash; 213 lines</summary>

```rust
11:pub
34:pub
40:pub
47:pub
60:fn read_device
73:fn read_volume
86:fn parse_mixer_vol
108:fn parse_sndstat_units
128:fn default_unit
134:fn device_name
138:fn run_mixer
149:fn run
162:fn read_sndstat
171:    fn mixer_vol_parses_level_and_mute
188:    fn mixer_vol_missing_fields_are_safe
193:    fn sndstat_lists_playback_units
205:    fn sndstat_skips_record_only_and_dedups
```

</details>

<details><summary><code>crates/meridian-shell/src/audio/mod.rs</code> &mdash; 189 lines</summary>

```rust
14:pub
23:pub
29:pub
37:impl AudioSnapshot
38:    pub
42:    pub
56:    pub
60:    pub
73:    pub
92:pub
97:pub
102:pub
107:fn backend_poll
111:fn backend_set_volume
115:fn backend_toggle_mute
119:fn backend_set_default
124:fn backend_poll
128:fn backend_set_volume
132:fn backend_toggle_mute
136:fn backend_set_default
145:    fn unavailable_snapshot_uses_muted_panel_fallback
153:    fn unavailable_snapshot_is_not_settled
159:    fn running_with_default_output_is_settled
178:    fn running_without_default_output_is_not_settled
```

</details>

<details><summary><code>crates/meridian-shell/src/audio/wpctl.rs</code> &mdash; 219 lines</summary>

```rust
7:pub
14:pub
18:pub
22:pub
26:fn parse_wpctl_status
41:fn parse_section_devices
63:fn parse_device_line
96:fn parse_volume_percent
103:fn run_wpctl_status
120:fn set_volume_args
129:fn set_mute_args
137:fn set_default_args
141:fn run_wpctl
154:    fn parse_wpctl_status_extracts_default_sink_and_source
174:    fn parse_wpctl_status_handles_real_unicode_tree_output
197:    fn set_volume_args_targets_default_sink_with_fractional_level
208:    fn set_mute_args_toggles_default_sink
216:    fn set_default_args_passes_numeric_id
```

</details>

<details><summary><code>crates/meridian-shell/src/audio_popup.rs</code> &mdash; 234 lines</summary>

```rust
15:pub enum AudioPopupHit
28:pub fn draw_audio_popup
92:pub fn draw_volume_osd
116:pub fn draw_text_osd
128:fn fit_text
137:pub fn popup_hit_test
161:pub fn volume_from_x
170:fn volume_from_bar_x
183:    fn render_for_test
194:    fn popup_hit_detection_reports_inside_and_outside
204:    fn volume_from_bar_x_maps_position_to_percent
220:    fn popup_hit_test_returns_settings_link_in_footer
```

</details>

<details><summary><code>crates/meridian-shell/src/autostart.rs</code> &mdash; 160 lines</summary>

```rust
3:pub fn launch_autostart_apps
53:struct DesktopSpec
58:fn parse_desktop_file
104:fn parse_exec
137:fn is_field_code
144:fn autostart_dirs
```

</details>

<details><summary><code>crates/meridian-shell/src/battery.rs</code> &mdash; 172 lines</summary>

```rust
9:pub enum ChargeState
18:pub struct BatterySnapshot
28:impl Default for BatterySnapshot
29:    fn default
41:impl BatterySnapshot
42:    pub fn poll
78:    pub fn icon_name
99:    pub fn label
104:fn read_trim
130:    fn icon_buckets_pick_level_and_charging
157:    fn label_is_percent
168:    fn poll_does_not_panic
```

</details>

<details><summary><code>crates/meridian-shell/src/bluetooth.rs</code> &mdash; 254 lines</summary>

```rust
15:pub struct BluetoothDevice
24:pub struct BluetoothSnapshot
32:impl BluetoothSnapshot
35:    pub fn poll
59:fn paired_addresses
66:fn connected_addresses
74:pub
84:pub
108:pub
121:pub
132:pub
145:pub
150:pub fn set_power
156:pub fn start_scan
163:pub fn pair_device
173:pub fn connect_device
177:fn run_bluetoothctl
189:fn run_bluetoothctl_background
195:fn run_bluetoothctl_blocking
213:    fn parse_show_flag_reads_yes_no
221:    fn parse_devices_flags_paired_and_connected
235:    fn parse_device_addresses_extracts_addrs
248:    fn argv_builders
```

</details>

<details><summary><code>crates/meridian-shell/src/buffer.rs</code> &mdash; 78 lines</summary>

```rust
5:pub fn shm_buffer_format
9:pub fn shm_buffer_stride
13:pub fn shm_buffer_size
17:pub fn buffer_for<'a>
65:    fn buffer_format_is_argb8888
70:    fn buffer_size_matches_dimensions
75:    fn buffer_stride_is_width_times_4
```

</details>

<details><summary><code>crates/meridian-shell/src/context_menu.rs</code> &mdash; 341 lines</summary>

```rust
22:fn menu_radius
29:fn is_glass_menu
33:fn menu_palette_from_config
37:fn ui_color
41:fn theme_accent_idle
45:fn theme_accent_hover
49:fn paint_menu_background
62:pub
71:pub
76:pub
86:pub
95:pub
109:pub
119:pub
128:pub
139:pub
163:pub
173:pub
185:pub
190:pub
208:pub
219:pub
227:pub
237:pub
245:pub
252:pub
266:pub
275:pub
281:pub
299:fn hit_item_at
316:pub
324:fn icon_for_desktop
```

</details>

<details><summary><code>crates/meridian-shell/src/context_menu/icons.rs</code> &mdash; 215 lines</summary>

```rust
1:fn draw_menu_icon
152:fn draw_submenu_arrow_indicator
194:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/context_menu/overlays.rs</code> &mdash; 319 lines</summary>

```rust
5:fn draw_overlay_with_background
124:pub
184:fn draw_submenu_overlay
274:fn blit_over
```

</details>

<details><summary><code>crates/meridian-shell/src/context_menu_tests.rs</code> &mdash; 236 lines</summary>

```rust
3:fn state
17:fn item_list_non_terminal_non_pinned_has_five_items
28:fn item_list_terminal_pinned_has_four_items
38:fn item_list_non_terminal_pinned_shows_unpin
46:fn hit_item_above_menu_is_none
53:fn hit_item_first_row
62:fn hit_item_last_row
73:fn contains_point_outside_returns_false
84:fn clamp_position_fits_inside_launcher
91:fn clamp_position_right_edge_clamped
97:fn clamp_position_bottom_edge_flips_up
104:fn draw_overlay_does_not_panic
121:fn draw_overlay_modifies_canvas_at_menu_location
144:fn desktop_item_list_has_five_items_with_settings_at_idx_three
158:fn submenu_items_has_expected_categories
171:fn submenu_hit_item_local_returns_correct_index
179:fn glass_menu_palette_uses_theme_colors
199:fn submenu_hit_item_local_hits_every_row_and_rejects_gap
220:fn total_menu_width_grows_when_submenu_open
226:fn desktop_hit_item_uses_desktop_coordinates
```

</details>

<details><summary><code>crates/meridian-shell/src/cursor.rs</code> &mdash; 126 lines</summary>

```rust
43:pub fn current_cursor_theme
53:fn cursor_theme_dirs
66:pub fn scan_cursor_themes
71:fn scan_dirs_for_cursor_themes
97:    fn size_options_ids_match_prefix_and_value
105:    fn theme_widget_ids_match_prefix_and_index
112:    fn scan_cursor_themes_finds_dirs_with_cursors_subdir_only
```

</details>

<details><summary><code>crates/meridian-shell/src/default_apps.rs</code> &mdash; 576 lines</summary>

```rust
29:pub enum DefaultAppCategory
41:impl DefaultAppCategory
54:    pub fn label
70:    pub fn representative_mime
87:    pub fn all_mimes
147:    pub fn preferred_desktop_ids
231:pub struct MimeAppCandidate
246:pub struct MimeAppIndex
251:impl MimeAppIndex
252:    pub fn load_system
289:    pub fn apps_for_mime
300:    pub fn lookup
309:pub fn snapshot_current_defaults
324:pub fn pick_file_manager
344:pub fn query_default
363:pub fn set_default_for_mimes
384:pub fn apply_sensible_defaults_for_empty
415:fn desktop_app_dirs
437:fn parse_mime_candidate
497:    fn category_metadata_matches_per_variant
521:    fn parse_minimal_desktop_entry_with_mimes
544:    fn parse_hidden_entry_is_skipped
554:    fn index_apps_for_mime_returns_only_matching
```

</details>

<details><summary><code>crates/meridian-shell/src/draw/bitmap.rs</code> &mdash; 76 lines</summary>

```rust
5:pub
30:fn bitmap_glyph
```

</details>

<details><summary><code>crates/meridian-shell/src/draw/mod.rs</code> &mdash; 6 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-shell/src/draw/painter.rs</code> &mdash; 343 lines</summary>

```rust
12:pub struct Painter<'a>
19:    pub fn new
27:    pub fn clear
34:    pub fn roundish_rect
38:    pub fn roundish_rect_with_radius
42:    pub fn rect
58:    fn fill_rounded_rect
127:    fn fill_pixel
139:    pub fn stroke_rect
178:    pub fn text_centered
195:    pub fn text_right_aligned
213:    pub fn text_clipped
233:    pub fn blend_pixel
252:    pub fn draw_image
299:fn clamped_radius
306:fn argb
313:fn premul_component
317:fn corner_coverage
```

</details>

<details><summary><code>crates/meridian-shell/src/draw/painter_tests.rs</code> &mdash; 324 lines</summary>

```rust
8:fn pixel_at
13:fn min_lit_x
30:fn radius_is_clamped_to_half_extent
36:fn radius_zero_behaves_like_rect_fill
56:fn rounded_corners_clip_outer_pixels
75:fn tiny_rectangles_are_handled_consistently
130:fn rounded_fill_uses_premultiplied_alpha_write_semantics
148:fn rounded_fill_full_coverage_pixels_are_premultiplied
166:fn rounded_fill_outside_corner_pixels_remain_untouched
183:fn rounded_fill_edge_pixels_use_partial_blending
208:fn corner_coverage_reports_expected_partial_and_full_values
216:fn text_centered_uses_fallback_measurement_when_font_missing
232:fn text_right_aligned_uses_right_padding_when_font_missing
248:fn draw_image_blits_centered_pixels
274:fn draw_image_clips_out_of_bounds_without_panic
301:fn draw_image_alpha_blending_respects_transparent_and_opaque_pixels
```

</details>

<details><summary><code>crates/meridian-shell/src/draw/text.rs</code> &mdash; 120 lines</summary>

```rust
16:pub struct TextRenderer
21:impl TextRenderer
26:    pub fn new
38:    pub fn draw_text
88:    pub fn measure_text
101:    fn test_renderer_creates_from_embedded_font
106:    fn measure_text_is_monotonic_for_longer_strings
116:    fn measure_text_empty_is_zero
```

</details>

<details><summary><code>crates/meridian-shell/src/font_resolve.rs</code> &mdash; 148 lines</summary>

```rust
19:type FcChar8 = c_uchar;
20:type FcBool = c_int;
21:enum FcConfig
22:enum FcPattern
23:type FcResult = c_int;
30:    fn FcInit
31:    fn FcNameParse
32:    fn FcConfigSubstitute
33:    fn FcDefaultSubstitute
34:    fn FcFontMatch
39:    fn FcPatternGetString
45:    fn FcPatternDestroy
48:fn resolve_family
91:fn family_from_pattern
103:pub
111:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/cache.rs</code> &mdash; 321 lines</summary>

```rust
5:enum CacheEntry
10:pub struct IconCache
15:impl IconCache
17:    pub fn new
21:    pub fn new_for_theme
29:    pub
36:    pub fn warm
61:    pub fn loader_config
70:    pub fn load_batch
93:    pub fn insert_loaded
104:    pub fn lookup
126:    struct TempDir
130:    impl TempDir
131:        fn new
145:        fn path
150:    impl Drop for TempDir
151:        fn drop
156:    fn write_theme_index
173:    fn write_png
186:    fn write_svg
191:    fn create_loader_env
205:    fn warm_is_idempotent_for_existing_key
232:    fn lookup_without_warm_returns_none
239:    fn missing_icon_is_negative_cached_after_warm
253:    fn lookup_returns_stable_reference_without_clone
269:    fn absolute_path_warm_finds_file
284:    fn absolute_path_warm_missing_file_negative_cache
295:    fn absolute_path_warm_downscale_resizes_to_requested
311:    fn absolute_path_svg_warm_finds_file
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/loader.rs</code> &mdash; 483 lines</summary>

```rust
21:pub
30:struct RccSource
36:struct ThemeLocation
42:struct IconCandidate
49:struct RccCandidate
55:impl IconLoader
59:    pub
63:    pub
84:    pub
99:    pub
111:    fn load_icon_by_name
122:    pub
133:    fn load_icon_from_theme
170:    fn load_from_rcc
194:    fn load_theme
209:    fn theme_chain
246:    fn load_from_pixmaps
256:    fn decode_icon_path
268:    fn decode_svg_file
276:    fn decode_png_file
281:    fn decode_icon_bytes
300:    fn decode_png_bytes
344:fn icon_aliases
351:fn discover_rcc_sources
397:fn select_rcc_candidates
423:fn pick_best_rcc_candidate
454:fn split_basename_and_extension
465:fn infer_nominal_size
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/loader/decode.rs</code> &mdash; 127 lines</summary>

```rust
1:fn decode_to_rgba8
66:fn rgba_to_bgra
77:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/loader/filesystem.rs</code> &mdash; 119 lines</summary>

```rust
1:impl IconLoader
3:    pub
8:    pub
16:pub
48:fn dedupe_paths
59:fn directory_matches_size
71:fn pick_best_candidate
96:fn choose_nearest
105:fn choose_smallest_size
111:fn choose_largest_size
117:fn distance
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/loader_tests.rs</code> &mdash; 521 lines</summary>

```rust
11:struct TempDir
15:impl TempDir
16:    fn new
30:    fn path
35:impl Drop for TempDir
36:    fn drop
41:fn write_theme_index
58:fn write_png_rgba
71:fn write_png_indexed
85:fn write_svg_solid_rect
92:fn make_theme_root
101:fn build_chain_rcc
169:fn write_rcc_file
174:fn lookup_finds_icon_in_theme_directory
198:fn lookup_falls_back_to_default_theme_before_hicolor
225:fn lookup_uses_terminal_alias_for_mini_xterm
247:fn absolute_path_loader_loads_png
262:fn svg_in_theme_directory_is_loaded
283:fn prefers_png_over_svg_in_same_directory
299:fn falls_back_to_svg_when_no_png
320:fn absolute_path_loader_loads_svg
334:fn size_selection_prefers_closest_larger_before_upscaling_smaller
367:fn inheritance_chain_finds_parent_theme_icon
390:fn pixmaps_fallback_is_used_when_theme_lookup_misses
411:fn downscale_produces_requested_dimensions
434:fn indexed_palette_png_is_skipped_without_panic
454:fn rcc_archives_are_discovered_in_theme_directory
472:fn lookup_falls_back_to_rcc_after_fs_exhausted
494:fn lookup_prefers_fs_over_rcc_when_both_present
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/mod.rs</code> &mdash; 61 lines</summary>

```rust
10:pub struct IconImage
16:pub
30:pub fn lookup_default_theme
39:    fn default_theme_is_breeze
46:    fn icon_image_to_pixmap_bgra_to_premul
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/rcc.rs</code> &mdash; 281 lines</summary>

```rust
14:pub
23:struct Header
30:enum RccNode
43:impl RccArchive
44:    pub
50:    fn from_test_bytes
54:    fn from_bytes
81:    pub
100:    pub
106:    fn collect_files
139:    fn find_child_by_name
164:    fn raw_file_payload
172:    fn parse_node
200:    fn node_name
208:    fn read_name
228:fn decode_payload
241:fn parse_header
267:fn read_u16_be
273:fn read_u32_be
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/rcc_tests.rs</code> &mdash; 342 lines</summary>

```rust
9:struct TestNode
15:enum TestNodeKind
27:fn build_rcc
92:fn minimal_root_with_file
110:fn build_rcc_data_names_tree
177:fn parse_header_returns_none_for_wrong_magic
188:fn parse_header_returns_none_for_wrong_version
199:fn parse_header_accepts_v3_with_expected_offsets
208:fn tree_walk_finds_root_directory_listing
215:fn tree_walk_navigates_subdirectories
251:fn tree_walk_works_when_tree_is_last_section
275:fn read_file_decompresses_zstd_payload
291:fn read_file_rejects_zlib_compressed_files
302:fn lookup_nonexistent_path_returns_none
311:fn integration_real_breeze_rcc_can_be_opened_and_query_known_icon
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/svg.rs</code> &mdash; 264 lines</summary>

```rust
8:pub fn decode_svg
20:pub fn decode_svg_with_symbolic_color
68:fn parse_hex_rgb
79:fn substitute_color_scheme
113:fn is_hex_color
119:fn memchr_contains
125:fn premultiplied_rgba_to_bgra_nonpremul
140:fn unpremultiply_channel
152:    fn pixel_at
163:    fn decode_svg_minimal_valid_returns_image
174:    fn decode_svg_renders_at_requested_size
182:    fn decode_svg_invalid_returns_none
187:    fn decode_svg_empty_returns_none
192:    fn decode_svg_size_zero_returns_none
198:    fn decode_svg_alpha_unpremultiplied
207:    fn decode_svg_substitutes_breeze_color_scheme
222:    fn decode_svg_uses_custom_symbolic_color
237:    fn symbolic_recolor_is_iconset_independent
259:    fn substitute_color_scheme_no_breeze_marker_is_noop
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/theme_index.rs</code> &mdash; 228 lines</summary>

```rust
4:pub
11:pub
21:pub
27:pub
110:fn split_csv
119:fn parse_u32
128:    fn parses_minimal_theme_with_inherits_and_directories
154:    fn parses_fixed_scalable_and_threshold_directory_types
187:    fn directory_defaults_follow_spec_for_threshold_and_type
207:    fn missing_optional_fields_do_not_fail_parse
```

</details>

<details><summary><code>crates/meridian-shell/src/launcher.rs</code> &mdash; 553 lines</summary>

```rust
16:pub struct DesktopApp
27:impl DesktopApp
28:    pub fn load_system
32:    pub
49:    fn load_from_dirs
78:    fn from_file
89:    fn from_desktop_entry_str_with_reason
194:pub struct LauncherState
199:impl LauncherState
200:    pub fn new_with_apps
209:    pub fn toggle
214:    pub fn close
218:    pub fn reshuffle
234:    pub
295:fn parse_categories
303:fn normalize_icon_name
320:fn parse_exec_argv
330:fn tokenize_exec
332:    enum Quote
389:fn strip_field_codes
417:fn argv_to_display
430:fn desktop_env_list_contains
438:fn cmp_apps
445:fn is_desktop_file
451:fn desktop_app_dirs
481:fn push_unique_dir
487:fn is_executable_available
504:fn is_executable_file
521:fn terminal_program
542:fn is_firefox_program
```

</details>

<details><summary><code>crates/meridian-shell/src/launcher_tests.rs</code> &mdash; 214 lines</summary>

```rust
12:struct TempDir
16:impl TempDir
17:    fn new
31:    fn path
36:impl Drop for TempDir
37:    fn drop
43:fn make_executable
52:fn parses_valid_desktop_entry
73:fn rejects_hidden_nodisplay_and_non_application_entries
91:fn desktop_visibility_respects_meridian_environment_keys
111:fn exec_field_codes_are_removed
119:fn exec_quotes_are_handled
127:fn parses_categories_and_normalizes_icon_names
144:fn try_exec_rejects_missing_binary
157:fn load_from_dirs_deduplicates_sorts_and_checks_try_exec
203:fn launcher_state_tracks_open_close_and_apps
```

</details>

<details><summary><code>crates/meridian-shell/src/main.rs</code> &mdash; 552 lines</summary>

```rust
125:pub
154:fn install_panic_logger
189:fn chrono_now
198:fn main
239:fn activate_user_session
264:fn redraw_after_ipc
296:fn insert_ipc_event_source
325:fn insert_status_notifier_source
376:fn insert_updates_refresh_ping
404:fn insert_notification_expiry_timer
435:fn insert_notifications_source
481:fn insert_tick_timer
510:fn insert_network_poll_timer
539:    fn default_pinned_apps_contains_expected_entries
```

</details>

<details><summary><code>crates/meridian-shell/src/network/freebsd.rs</code> &mdash; 584 lines</summary>

```rust
17:pub struct ConnectionProfile
25:pub struct WifiNetwork
32:pub struct NetworkController
36:impl NetworkController
37:    pub fn new
43:    pub fn poll
51:    pub fn state
58:struct IfaceSummary
67:impl IfaceSummary
68:    fn is_wifi
76:fn classify_kind
92:fn kind_priority
103:pub
139:fn parse_ifconfig_interfaces
192:fn parse_ssid_line
214:pub fn list_saved_connections
221:pub
245:pub fn scan_wifi_networks
258:pub
305:fn find_bssid
326:fn rssi_to_percent
331:fn first_wlan_interface
341:pub fn activate_connection
350:pub fn connect_wifi
370:fn wpa_cli_connect
407:fn run_ifconfig
447:    fn parses_wired_connected
458:    fn parses_wifi_with_ssid
469:    fn wired_wins_over_wifi
481:    fn down_interface_is_disconnected
493:    fn loopback_only_is_offline
502:    fn quoted_ssid_with_spaces
515:    fn saved_connections_lists_non_loopback
524:    fn scan_parses_ssid_signal_and_security
542:    fn rssi_scales_to_percent
550:    fn classify_kind_maps_prefixes
563:    fn vpn_chosen_when_only_tunnel_up
579:    fn find_bssid_locates_mac
```

</details>

<details><summary><code>crates/meridian-shell/src/network/mod.rs</code> &mdash; 184 lines</summary>

```rust
22:pub enum NetworkState
32:pub enum ConnectionKind
39:impl NetworkState
40:    pub fn icon_name
59:    pub fn settings_rows
92:    fn settings_rows_summarize_state
118:    fn icon_name_for_each_variant
```

</details>

<details><summary><code>crates/meridian-shell/src/network/nmcli.rs</code> &mdash; 470 lines</summary>

```rust
30:pub struct ConnectionProfile
39:pub struct WifiNetwork
49:pub struct NetworkController
53:impl NetworkController
54:    pub fn new
60:    pub fn poll
93:    pub fn state
98:pub
188:pub fn list_saved_connections
198:pub
235:pub
247:pub fn activate_connection
255:pub fn scan_wifi_networks
265:pub
315:pub
320:impl NmcliInvocation
321:    fn args
329:pub
346:pub fn connect_wifi
352:fn run_nmcli_background
396:fn redact_nmcli_args
413:fn run_nmcli
425:fn parse_terse_fields
449:fn parse_wifi_signal_from_scan
```

</details>

<details><summary><code>crates/meridian-shell/src/network/nmcli_tests.rs</code> &mdash; 195 lines</summary>

```rust
8:fn parse_state_returns_offline_for_unparsable_general
14:fn parse_state_returns_disconnected_when_general_is_not_connected
23:fn parse_state_returns_ethernet_when_wired_connected
38:fn parse_state_returns_wifi_when_wireless_connected
50:fn parse_state_ignores_loopback
59:fn parse_state_strips_quoted_colons_in_connection_names
71:fn parse_wifi_signal_finds_active_network
77:fn parse_saved_connections_marks_active_and_skips_loopback
93:fn parse_saved_connections_handles_quoted_colons_in_name
100:fn activate_connection_args_uses_id_form
108:fn parse_wifi_networks_dedups_sorts_and_flags_security
132:fn connect_wifi_invocation_uses_stdin_for_passwords
156:fn redact_nmcli_args_hides_password_values
179:fn integration_real_nmcli_can_be_polled
```

</details>

<details><summary><code>crates/meridian-shell/src/network_popup.rs</code> &mdash; 411 lines</summary>

```rust
17:pub enum NetworkTab
23:pub enum NetworkPopupHit
47:pub fn draw_network_popup
75:fn draw_status_tab
126:fn draw_wifi_tab
195:fn draw_tabs
240:fn draw_tab
269:fn measure
276:pub fn popup_hit_test
309:    fn render
326:    fn net
336:    fn popup_hit_detection_reports_inside_and_outside
346:    fn popup_hit_test_returns_settings_link_in_footer
359:    fn tabs_are_hit_testable
376:    fn wifi_rows_hit_test_to_their_index
398:    fn switching_to_status_tab_clears_wifi_row_targets
```

</details>

<details><summary><code>crates/meridian-shell/src/notification_popup.rs</code> &mdash; 106 lines</summary>

```rust
17:pub fn draw_notification
```

</details>

<details><summary><code>crates/meridian-shell/src/notifications/dbus.rs</code> &mdash; 170 lines</summary>

```rust
30:pub enum DbusEvent
39:struct NotificationsService
48:impl NotificationsService
51:    async fn notify
93:    async fn close_notification
104:    async fn get_capabilities
109:    async fn get_server_information
124:pub fn spawn
151:async fn run
```

</details>

<details><summary><code>crates/meridian-shell/src/notifications/mod.rs</code> &mdash; 15 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-shell/src/notifications/state.rs</code> &mdash; 139 lines</summary>

```rust
17:pub enum Urgency
24:impl Urgency
25:    pub fn from_byte
37:pub struct Notification
56:impl Notification
58:    pub fn is_expired
68:pub fn expires_in_from_timeout
81:    fn timeout_zero_means_never
86:    fn timeout_negative_means_default
94:    fn timeout_positive_used_as_is
102:    fn urgency_byte_mapping
111:    fn expired_when_ttl_elapsed
126:    fn never_expires_with_none
```

</details>

<details><summary><code>crates/meridian-shell/src/panel.rs</code> &mdash; 29 lines</summary>

```rust
3:pub struct PanelState
8:pub struct PinnedApp
17:pub struct PanelWindowEntry
25:impl PanelState
26:    pub fn new
```

</details>

<details><summary><code>crates/meridian-shell/src/panel_view.rs</code> &mdash; 302 lines</summary>

```rust
82:fn build_launcher_icon
146:fn build_audio_icon
234:fn action_for_id_as_click
253:fn status_notifier_label
```

</details>

<details><summary><code>crates/meridian-shell/src/panel_view/chips.rs</code> &mdash; 165 lines</summary>

```rust
1:struct PanelDivider;
3:impl Widget for PanelDivider
4:    fn style
14:    fn paint
29:struct PanelChip
37:impl PanelChip
38:    fn new
55:impl Widget for PanelChip
56:    fn id
60:    fn style
70:    fn paint
```

</details>

<details><summary><code>crates/meridian-shell/src/panel_view/layout.rs</code> &mdash; 309 lines</summary>

```rust
1:fn draw_circle
21:fn windows_for_pinned_app
55:fn tint_pixmap_premul
65:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/panel_view/pinned.rs</code> &mdash; 158 lines</summary>

```rust
1:struct PanelPinnedChip
11:impl Widget for PanelPinnedChip
12:    fn id
16:    fn pinned_app_idx
20:    fn launch_info
24:    fn style
34:    fn paint
```

</details>

<details><summary><code>crates/meridian-shell/src/panel_view/render.rs</code> &mdash; 228 lines</summary>

```rust
1:fn collect_click_zones
45:fn apply_frost_noise
63:fn blit_rgba_to_argb
76:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/panel_view/windows.rs</code> &mdash; 86 lines</summary>

```rust
5:struct PanelWindowChip
14:impl Widget for PanelWindowChip
15:    fn id
19:    fn focus_window_id
23:    fn style
33:    fn paint
```

</details>

<details><summary><code>crates/meridian-shell/src/panel_view_tests.rs</code> &mdash; 172 lines</summary>

```rust
7:fn panel_chip_style_returns_correct_size
15:fn tray_chip_widths_match
22:fn panel_pinned_chip_pinned_app_idx_returns_idx
36:fn panel_pinned_chip_launch_info_returns_program_and_args
50:fn panel_window_chip_focus_window_id_returns_id
62:fn status_notifier_label_prefers_title_then_icon_then_service
93:fn build_panel_widget_tree_root_has_three_children
125:fn draw_panel_ui_modifies_canvas_and_fills_clicks
163:fn action_for_id_as_click_screenshot
```

</details>

<details><summary><code>crates/meridian-shell/src/popup_card.rs</code> &mdash; 521 lines</summary>

```rust
21:fn card_radius
43:pub fn draw_card_body
48:fn draw_card_border
77:fn rounded_rect_coverage
102:fn rounded_rect_sample_inside
130:pub fn draw_glass_card_border_in_rect_with_color
141:fn draw_card_border_in_rect
195:pub fn paint_card_with_shadow
225:pub fn paint_card_panels_with_shadow
267:pub fn draw_card_title
300:pub fn draw_kv_row
335:pub fn draw_status_row
387:pub fn draw_volume_row
490:pub fn draw_footer_link
```

</details>

<details><summary><code>crates/meridian-shell/src/power_profile.rs</code> &mdash; 86 lines</summary>

```rust
8:pub enum PowerProfile
14:impl PowerProfile
22:    pub fn daemon_id
31:    pub fn label
39:    pub fn from_daemon
50:pub fn current
59:pub fn set
73:    fn maps_user_names_to_daemon_ids
80:    fn round_trips_through_daemon_id
```

</details>

<details><summary><code>crates/meridian-shell/src/printers.rs</code> &mdash; 257 lines</summary>

```rust
5:pub
13:pub
20:pub
29:impl PrinterSnapshot
30:    pub
80:pub
114:fn parse_printer_line
147:fn parse_default_printer
159:fn parse_accepting
174:fn parse_jobs
200:struct CommandOutput
205:fn run_lpstat
222:    fn parse_snapshot_extracts_default_status_and_jobs
244:    fn parse_snapshot_handles_no_default_and_empty_printer_list
```

</details>

<details><summary><code>crates/meridian-shell/src/region_picker.rs</code> &mdash; 342 lines</summary>

```rust
44:pub
51:impl RegionRect
52:    pub
65:pub
91:pub
106:fn fill_dim
120:fn clear_rect_interior
147:fn draw_rect_border
183:fn paint_status
221:fn blit_text
269:    fn rect_from_drag_normalises_direction
295:    fn rect_from_drag_clamps_to_canvas
319:    fn rect_from_drag_returns_none_for_zero_area
329:    fn to_screenshot_region_preserves_geometry
```

</details>

<details><summary><code>crates/meridian-shell/src/screenshot_consent.rs</code> &mdash; 259 lines</summary>

```rust
29:pub
36:fn button_rects
56:pub
75:pub
170:fn fit
180:fn blit
225:    fn hit_button_distinguishes_allow_and_deny
241:    fn hit_button_misses_outside_buttons
252:    fn buttons_lie_within_the_modal
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view.rs</code> &mdash; 459 lines</summary>

```rust
40:pub enum SettingsCategory
58:impl SettingsCategory
79:    pub fn label
98:    pub fn chip_id
119:    pub fn search_keywords
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/appearance_widgets.rs</code> &mdash; 245 lines</summary>

```rust
1:struct WallpaperRow
10:impl Widget for WallpaperRow
11:    fn id
15:    fn style
25:    fn paint
94:struct WallpaperBrowseRow
99:impl Widget for WallpaperBrowseRow
100:    fn id
104:    fn style
114:    fn paint
157:struct PinnedAppLabel
164:impl Widget for PinnedAppLabel
165:    fn id
169:    fn style
179:    fn paint
219:struct SettingsPlaceholder
224:impl Widget for SettingsPlaceholder
225:    fn style
235:    fn paint
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/audio_system_widgets.rs</code> &mdash; 488 lines</summary>

```rust
1:struct SoundSummaryCard
7:impl Widget for SoundSummaryCard
8:    fn style
18:    fn paint
77:struct SoundDeviceRow
89:impl Widget for SoundDeviceRow
90:    fn id
99:    fn style
109:    fn paint
187:struct PrinterSummaryCard
193:impl Widget for PrinterSummaryCard
194:    fn style
204:    fn paint
267:struct SystemInfoRow
273:impl Widget for SystemInfoRow
274:    fn style
284:    fn paint
324:fn default_apps_pick_id
336:fn default_apps_set_id
358:struct DefaultAppCategoryRow
367:impl Widget for DefaultAppCategoryRow
368:    fn id
371:    fn style
380:    fn paint
438:struct DefaultAppCandidateRow
447:impl Widget for DefaultAppCandidateRow
448:    fn id
451:    fn style
460:    fn paint
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/basic_widgets.rs</code> &mdash; 414 lines</summary>

```rust
1:fn with_alpha
5:fn settings_glass_theme_from_config
16:struct SettingsHeaderBar
21:impl Widget for SettingsHeaderBar
22:    fn style
34:    fn paint
40:    fn children
47:struct SettingsBackButton;
49:impl Widget for SettingsBackButton
50:    fn id
54:    fn style
64:    fn paint
84:struct SettingsSearchField
89:impl Widget for SettingsSearchField
90:    fn style
100:    fn paint
121:struct SidebarPanel
128:impl Widget for SidebarPanel
129:    fn style
148:    fn paint
154:    fn children
160:struct SidebarSectionLabel
166:impl Widget for SidebarSectionLabel
167:    fn style
177:    fn paint
189:struct SettingsSidebarRow
196:impl Widget for SettingsSidebarRow
197:    fn id
201:    fn style
211:    fn paint
253:struct VerticalDivider
258:impl Widget for VerticalDivider
259:    fn style
269:    fn paint
276:struct ThemeRow
284:impl Widget for ThemeRow
285:    fn id
289:    fn style
299:    fn paint
344:struct CursorThemeRow
352:impl Widget for CursorThemeRow
353:    fn id
359:    fn style
369:    fn paint
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/bluetooth.rs</code> &mdash; 101 lines</summary>

```rust
1:fn build_bluetooth_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/cursor.rs</code> &mdash; 60 lines</summary>

```rust
1:fn build_cursor_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/default_apps.rs</code> &mdash; 111 lines</summary>

```rust
1:fn build_default_apps_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/display.rs</code> &mdash; 118 lines</summary>

```rust
1:fn build_display_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/network.rs</code> &mdash; 85 lines</summary>

```rust
1:fn build_network_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/pinned_apps.rs</code> &mdash; 140 lines</summary>

```rust
1:fn build_pinned_apps_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/power.rs</code> &mdash; 47 lines</summary>

```rust
1:fn build_power_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/printers.rs</code> &mdash; 37 lines</summary>

```rust
1:fn build_printers_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/sound.rs</code> &mdash; 93 lines</summary>

```rust
1:fn build_sound_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/system_overview.rs</code> &mdash; 23 lines</summary>

```rust
1:fn build_system_overview_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/theme.rs</code> &mdash; 35 lines</summary>

```rust
1:fn build_theme_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/updates.rs</code> &mdash; 21 lines</summary>

```rust
1:fn build_updates_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/users.rs</code> &mdash; 22 lines</summary>

```rust
1:fn build_users_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content/wallpaper.rs</code> &mdash; 81 lines</summary>

```rust
1:fn build_wallpaper_content
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/content_builders.rs</code> &mdash; 257 lines</summary>

```rust
16:struct SettingsContentContext<'a>
50:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/display_controls.rs</code> &mdash; 373 lines</summary>

```rust
1:struct DisplayModeComboButton
9:impl Widget for DisplayModeComboButton
10:    fn id
18:    fn style
28:    fn paint
85:struct DisplayModeOptionRow
94:impl Widget for DisplayModeOptionRow
95:    fn id
102:    fn style
112:    fn paint
153:struct DisplayPrimaryButton
159:impl Widget for DisplayPrimaryButton
160:    fn id
168:    fn style
178:    fn paint
241:struct DisplayCycleButton
248:impl Widget for DisplayCycleButton
249:    fn id
253:    fn style
263:    fn paint
292:struct AddAppRow
300:impl Widget for AddAppRow
301:    fn id
305:    fn style
315:    fn paint
352:struct Divider
357:impl Widget for Divider
358:    fn style
368:    fn paint
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/display_widgets.rs</code> &mdash; 213 lines</summary>

```rust
1:fn fit_text
10:struct DisplayOutputRow
28:impl Widget for DisplayOutputRow
29:    fn style
39:    fn paint
182:fn display_badge_text
191:fn display_mode_label
200:fn selected_display_mode
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/draw.rs</code> &mdash; 113 lines</summary>

```rust
2:pub
95:fn printer_service_message
103:fn blit_rgba_to_argb
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/ids.rs</code> &mdash; 317 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-shell/src/settings_view/network_device_widgets.rs</code> &mdash; 369 lines</summary>

```rust
1:struct NetworkProfileRow
10:impl Widget for NetworkProfileRow
11:    fn id
19:    fn style
29:    fn paint
95:struct WifiRow
105:impl Widget for WifiRow
106:    fn id
114:    fn style
124:    fn paint
192:struct BluetoothDeviceRow
202:impl Widget for BluetoothDeviceRow
203:    fn id
211:    fn style
221:    fn paint
290:struct PrinterRow
296:impl Widget for PrinterRow
297:    fn style
307:    fn paint
```

</details>

<details><summary><code>crates/meridian-shell/src/soft_shadow.rs</code> &mdash; 167 lines</summary>

```rust
20:pub
63:fn rounded_box_sdf
74:pub
147:    fn translucent_clip_keeps_the_actual_edge_pixel
```

</details>

<details><summary><code>crates/meridian-shell/src/status_notifier.rs</code> &mdash; 316 lines</summary>

```rust
12:type DbusMenuProperties = std::collections::HashMap<String, OwnedValue>;
13:type DbusMenuLayoutNode =
16:pub
24:pub enum DbusEvent
30:pub
38:pub
45:pub
54:pub
63:pub
68:impl DbusMenu
69:    pub
73:    pub
77:    pub
81:    pub
90:impl DbusMenuItem
91:    fn subtree_count
99:    fn actionable_count
109:    fn first_label
116:    fn push_display_entries
131:pub
137:enum ActivationKind
143:impl ActivationKind
144:    fn method_name
152:    fn log_name
162:struct WatcherState
167:struct StatusNotifierWatcher
173:impl StatusNotifierWatcher
174:    async fn register_status_notifier_item
201:    async fn register_status_notifier_host
213:    async fn registered_status_notifier_items
219:    async fn is_status_notifier_host_registered
224:    async fn protocol_version
229:impl StatusNotifierWatcher
230:    fn send_items
237:pub
242:pub fn spawn
270:pub
274:pub
278:pub
299:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/status_notifier/activation_and_menu.rs</code> &mdash; 318 lines</summary>

```rust
1:fn forward_item_activation
54:async fn run
73:async fn forward_activation
92:fn inspect_dbus_menu
160:async fn send_dbus_menu_event
169:async fn fetch_dbus_menu_layout
180:fn parse_dbus_menu_layout
194:fn parse_dbus_menu_item
219:fn property_string
226:fn property_bool
233:fn normalize_menu_label
249:fn normalize_service
253:async fn resolve_item_details
281:async fn read_string_property
297:async fn read_object_path_property
316:fn snapshot_items
```

</details>

<details><summary><code>crates/meridian-shell/src/status_notifier_popup.rs</code> &mdash; 189 lines</summary>

```rust
16:pub fn menu_height
21:pub fn visible_row_capacity
27:pub fn hit_item
44:pub fn draw_status_notifier_menu
159:    fn entry
170:    fn menu_height_grows_with_rows
176:    fn hit_item_returns_enabled_row_id
184:    fn hit_item_ignores_separators
```

</details>

<details><summary><code>crates/meridian-shell/src/status_notifier_tests.rs</code> &mdash; 154 lines</summary>

```rust
10:fn text_value
14:fn item_node
19:fn props
27:fn normalize_service_trims_input
35:fn snapshot_items_is_sorted_and_stable
75:fn normalize_menu_label_removes_mnemonics
82:fn parse_dbus_menu_layout_filters_hidden_items
112:fn parse_dbus_menu_layout_keeps_separators_and_children
137:fn display_entries_flattens_children_with_depth
```

</details>

<details><summary><code>crates/meridian-shell/src/sysinfo.rs</code> &mdash; 221 lines</summary>

```rust
12:pub struct SystemInfo
21:impl SystemInfo
22:    pub fn gather
77:    pub fn rows
89:fn parse_os_pretty_name
96:fn parse_uptime_seconds
104:fn format_uptime
117:fn parse_cpuinfo
138:fn format_cpu
147:fn parse_meminfo_kib
160:fn format_memory
168:fn gib
177:    fn os_pretty_name_handles_quotes_and_absence
188:    fn uptime_parses_first_field_and_formats
197:    fn cpuinfo_counts_processors_and_takes_first_model
207:    fn meminfo_parses_kib_and_formats_used_over_total
217:    fn missing_meminfo_fields_degrade_gracefully
```

</details>

<details><summary><code>crates/meridian-shell/src/theme_export.rs</code> &mdash; 396 lines</summary>

```rust
32:pub
80:fn config_home
85:fn data_home
89:fn abs_env
93:fn home
97:fn write_file
114:fn substitute_tokens
139:fn index_theme
154:pub
176:pub
210:fn push_color_group
232:fn apply_gsettings
244:fn set_gsetting
260:fn app_icon_theme
274:fn triplet
280:fn readable_on
294:    fn light_theme
303:    fn triplet_formats_decimal_rgb
309:    fn readable_on_picks_contrasting_role
316:    fn kdeglobals_dark_has_scheme_groups_and_breeze
325:    fn kdeglobals_light_switches_scheme_name
330:    fn gtk_ini_selects_meridian_theme_and_dark_flag
341:    fn gtk_ini_light_clears_dark_flag
348:    fn gtk_css_substitutes_all_tokens_with_hex
363:    fn config_gtk_css_carries_libadwaita_named_colours
374:    fn index_theme_names_meridian
381:    fn app_icon_theme_maps_papirus_to_dark_light_variant
```

</details>

<details><summary><code>crates/meridian-shell/src/thumbnail_popup.rs</code> &mdash; 198 lines</summary>

```rust
10:pub
32:pub
108:fn blit_xrgb
140:    fn draw_thumbnail_popup_does_not_panic_with_empty_cache
150:    fn popup_width_falls_back_to_max_for_uncached_ids
158:    fn popup_width_uses_cached_thumb_widths
167:    fn draw_thumbnail_popup_blits_thumbnail_pixels
```

</details>

<details><summary><code>crates/meridian-shell/src/ui/mod.rs</code> &mdash; 2 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-shell/src/ui/primitives.rs</code> &mdash; 25 lines</summary>

```rust
5:pub enum ActiveIndicatorEdge
9:pub fn draw_active_indicator
```

</details>

<details><summary><code>crates/meridian-shell/src/ui/tokens.rs</code> &mdash; 86 lines</summary>

```rust
7:pub
11:pub
15:pub
19:pub
23:pub
28:pub
33:pub
37:fn radius_scale_from_config
49:pub
66:pub
72:pub
80:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/updates.rs</code> &mdash; 132 lines</summary>

```rust
17:type UpdateRows = Vec<
26:pub fn set_refresh_ping
30:fn cache
37:pub fn updates_rows
58:fn query_blocking
67:fn parse_upgradable
77:fn rows_from
99:    fn parse_extracts_package_names
110:    fn parse_ignores_header_and_blanks
115:    fn rows_up_to_date_when_empty
123:    fn rows_count_and_overflow
```

</details>

<details><summary><code>crates/meridian-shell/src/users.rs</code> &mdash; 146 lines</summary>

```rust
14:pub struct LocalUser
20:pub struct UserAccounts
25:impl UserAccounts
26:    pub fn gather
39:    pub fn rows
57:fn parse_local_users
82:    fn parse_filters_to_human_uids
104:    fn parse_skips_malformed_lines
111:    fn rows_put_current_first_and_mark_active
137:    fn rows_handle_no_accounts
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/calendar.rs</code> &mdash; 186 lines</summary>

```rust
2:pub
7:pub
11:impl Default for CalendarDisplayPolicy
12:    fn default
19:pub
26:pub
34:impl CalendarMonthModel
35:    pub
67:fn days_in_month
76:fn is_leap_year
80:fn weekday_col0_from_sunday0
86:fn weekday_sunday0
114:    fn leap_year_february_has_29_days
119:    fn normal_february_has_28_days
124:    fn thirty_day_month_is_reported
129:    fn thirty_one_day_month_is_reported
134:    fn first_weekday_uses_monday_zero_mapping_when_monday_is_selected
148:    fn cells_place_days_in_expected_positions_for_monday_start
160:    fn today_day_is_kept_only_when_in_month_range
174:    fn invalid_month_is_rejected
180:    fn weekday_labels_follow_selected_week_start
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/compositor.rs</code> &mdash; 85 lines</summary>

```rust
10:impl CompositorHandler for MeridianShell
11:    fn scale_factor_changed
20:    fn transform_changed
29:    fn frame
68:    fn surface_enter
77:    fn surface_leave
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/keyboard.rs</code> &mdash; 458 lines</summary>

```rust
15:impl KeyboardHandler for MeridianShell
16:    fn enter
49:    fn leave
66:    fn press_key
438:    fn release_key
448:    fn update_modifiers
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/layer.rs</code> &mdash; 564 lines</summary>

```rust
16:impl LayerShellHandler for MeridianShell
17:    fn closed
132:    fn configure
528:impl MeridianShell
529:    fn panel_output_width_fallback
547:    fn output_height_fallback
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/mod.rs</code> &mdash; 38 lines</summary>

```rust
32:impl ProvidesRegistryState for MeridianShell
33:    fn registry
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/output.rs</code> &mdash; 34 lines</summary>

```rust
6:impl OutputHandler for MeridianShell
7:    fn output_state
11:    fn new_output
19:    fn update_output
27:    fn output_destroyed
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/pointer.rs</code> &mdash; 35 lines</summary>

```rust
21:impl PointerHandler for MeridianShell
22:    fn pointer_frame
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/pointer/launcher.rs</code> &mdash; 502 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/pointer/overlays_and_desktop.rs</code> &mdash; 192 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/pointer/panel_and_popups.rs</code> &mdash; 357 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/pointer_state.rs</code> &mdash; 342 lines</summary>

```rust
7:pub
35:pub
55:impl MeridianShell
60:    pub
95:    pub
122:    pub
160:    fn path
164:    fn pos
168:    fn path_a
172:    fn path_b
176:    fn root_path
181:    fn apply_move_with_hit_returns_hovered
190:    fn apply_move_without_hit_returns_none
199:    fn apply_press_left_returns_pressed
209:    fn apply_press_right_keeps_current
220:    fn apply_release_left_on_pressed_returns_hovered
236:    fn apply_release_left_off_target_returns_none
252:    fn apply_leave_returns_none
259:    fn apply_move_keeps_pressed_when_still_on_pressed_path
274:    fn apply_move_to_other_widget_while_pressed_switches_to_hovered_new
289:    fn apply_enter_on_empty_path_returns_hovered_at_root
298:    fn click_on_same_widget_detected
310:    fn click_on_different_widget_not_detected
322:    fn release_without_prior_press_not_click
333:    fn non_release_event_not_click
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/pointer_translate.rs</code> &mdash; 212 lines</summary>

```rust
8:fn translate_pointer_button
17:pub
52:    fn translate_button_left
60:    fn translate_button_right
68:    fn translate_button_middle
76:    fn translate_button_unknown
82:    fn translate_motion_yields_pointer_move_with_position
94:    fn translate_enter_yields_pointer_enter_with_position
106:    fn translate_leave_yields_pointer_leave
113:    fn translate_press_left_yields_pointer_press_left
130:    fn translate_press_right_yields_pointer_press_right
147:    fn translate_press_middle_yields_pointer_press_middle
164:    fn translate_press_unknown_button_yields_none
174:    fn translate_release_left_yields_pointer_release_left
191:    fn translate_axis_yields_none
202:    fn translate_position_truncates_to_i32
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/seat.rs</code> &mdash; 48 lines</summary>

```rust
6:impl SeatHandler for MeridianShell
7:    fn seat_state
11:    fn new_seat
13:    fn new_capability
28:    fn remove_capability
47:    fn remove_seat
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/shm.rs</code> &mdash; 9 lines</summary>

```rust
5:impl ShmHandler for MeridianShell
6:    fn shm_state
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/widget_dispatch.rs</code> &mdash; 59 lines</summary>

```rust
20:fn power_action_command
31:fn power_action_command
47:    fn power_off_maps_to_platform_command
56:    fn logout_has_no_external_command
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/widget_dispatch/dispatch.rs</code> &mdash; 468 lines</summary>

```rust
1:impl MeridianShell
2:    pub
71:    fn dispatch_launch_action
91:    fn dispatch_popup_action
118:    fn dispatch_settings_action
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/handlers/widget_dispatch/settings_and_context.rs</code> &mdash; 285 lines</summary>

```rust
1:impl MeridianShell
2:    fn apply_output_mode_selection
51:    fn dispatch_power_action
82:    fn dispatch_pinned_action
117:    fn persist_pinned_apps_and_redraw
123:    fn add_pinned_app_by_addable_index
156:    pub
210:    pub
227:    pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/init.rs</code> &mdash; 593 lines</summary>

```rust
34:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/init/assets.rs</code> &mdash; 79 lines</summary>

```rust
5:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/init/commit.rs</code> &mdash; 30 lines</summary>

```rust
6:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/init/flags.rs</code> &mdash; 20 lines</summary>

```rust
1:pub
17:    fn missing_flag_is_disabled
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/ipc.rs</code> &mdash; 289 lines</summary>

```rust
13:pub struct IpcClient
19:impl IpcClient
20:    pub
30:    pub
34:    pub
38:    pub
44:    pub
75:    pub
126:    pub fn send
149:    fn disconnect
156:fn shell_auth_command
166:fn parse_event_line
219:    fn oversized_incomplete_line_disconnects_and_clears_buffer
261:    fn json_event_parsing_remains_preferred
270:    fn legacy_workspace_changed_event_still_parses
278:    fn legacy_window_focus_cleared_event_still_parses
286:    fn invalid_line_is_ignored
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/mod.rs</code> &mdash; 19 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/render.rs</code> &mdash; 84 lines</summary>

```rust
31:fn german_month_name
52:fn round_buffer_corners
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/render/calendar_workspace.rs</code> &mdash; 306 lines</summary>

```rust
1:impl MeridianShell
2:    pub
202:    pub
212:    pub
297:    pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/render/core.rs</code> &mdash; 310 lines</summary>

```rust
1:impl MeridianShell
2:    fn signature_hash<T: Hash>
8:    fn theme_render_signature
36:    pub
58:    fn panel_render_signature
102:    fn commit_surface_label
109:    fn commit_reason_label
121:    fn commit_reason_from_repaint
135:    pub
158:    pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/render/desktop_overlays.rs</code> &mdash; 339 lines</summary>

```rust
1:impl MeridianShell
4:    pub
21:    pub
134:    pub
159:    pub
207:    pub
262:    pub
267:    pub
278:    pub
329:    pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/render/launcher.rs</code> &mdash; 250 lines</summary>

```rust
1:impl MeridianShell
2:    pub
239:    pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/render/network_audio.rs</code> &mdash; 263 lines</summary>

```rust
1:impl MeridianShell
2:    pub
79:    pub
94:    pub
169:    pub
253:    pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/render/notifications.rs</code> &mdash; 320 lines</summary>

```rust
1:impl MeridianShell
2:    pub
89:    pub
103:    pub
184:    pub
194:    pub
275:    pub
281:    fn write_panel_click_zones_snapshot
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/screencopy.rs</code> &mdash; 444 lines</summary>

```rust
24:fn create_screenshot_shm
28:            fn shm_mkstemp
62:pub
84:impl Drop for ScreenshotCapture
85:    fn drop
105:impl Dispatch<ExtImageCopyCaptureManagerV1,
106:    fn event
117:impl Dispatch<ExtOutputImageCaptureSourceManagerV1,
118:    fn event
129:impl Dispatch<ExtImageCaptureSourceV1,
130:    fn event
141:impl Dispatch<WlShmPool,
142:    fn event
153:impl Dispatch<WlBuffer,
154:    fn event
167:impl Dispatch<ExtImageCopyCaptureSessionV1,
168:    fn event
202:fn issue_frame_capture
276:fn crop_screenshot_region
302:fn is_supported_screenshot_format
306:fn screenshot_buffer_layout
320:impl Dispatch<ExtImageCopyCaptureFrameV1,
321:    fn event
383:pub
409:    fn xrgb_to_rgb_channel_swap
425:    fn screenshot_buffer_layout_rejects_empty_or_oversized_constraints
437:    fn openbsd_screenshot_shm_is_close_on_exec_and_resizable
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/shell.rs</code> &mdash; 481 lines</summary>

```rust
30:pub
39:pub
45:pub
56:pub
66:impl CommitReasonCounts
67:    pub
79:    pub
91:pub
96:impl CommitStats
97:    pub
104:    pub
108:    pub
112:    pub
118:pub
137:impl RepaintStats
138:    pub
149:    pub
160:    pub
164:    pub
170:pub
176:pub
197:pub
204:pub
209:impl ShellRenderStats
210:    pub
219:    pub
224:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/state.rs</code> &mdash; 419 lines</summary>

```rust
17:fn workspace_idx
21:fn normalize_workspace_1_based_u8
25:fn apply_workspace_changed
29:fn panel_global_activation_point
45:fn normalize_workspace_1_based
49:fn select_panel_active_workspace
94:fn apply_output_workspace_snapshot_state
114:struct OutputWorkspaceChangedInput
121:fn apply_output_workspace_changed_state
168:fn apply_window_opened_state
191:fn apply_window_closed_state
195:fn clear_stale_focused_window_id
205:fn apply_full_window_snapshot
229:fn compute_occupied_workspaces
237:fn panel_theme_signature
262:fn resolve_shell_theme_from_config
290:pub
306:pub
324:struct WallpaperPickerCommand
329:fn wallpaper_picker_command
349:fn app_matches_window
362:fn first_minimized_pinned_app_window_id
391:fn hidden_apps_path
396:fn load_wallpaper_thumbnail
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/state/audio_and_network_popups.rs</code> &mdash; 255 lines</summary>

```rust
1:impl MeridianShell
2:    pub
19:    pub
77:    pub
98:    pub
138:    pub
173:    pub
189:    pub
202:    pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/state/ipc_events.rs</code> &mdash; 374 lines</summary>

```rust
1:impl MeridianShell
2:    fn apply_ipc_event
199:    fn handle_config_reloaded
259:    fn update_focused_title
277:    fn warm_launcher_icons
312:    fn poll_launcher_icons_warm
338:    pub
351:    fn poll_launcher_apps_refresh
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/state/panel_actions.rs</code> &mdash; 357 lines</summary>

```rust
1:impl MeridianShell
2:    pub
148:    pub
164:    pub
181:    pub
202:    fn panel_output_height_fallback
220:    pub
243:    fn update_occupied_workspaces
258:    fn maybe_log_repaint_stats
291:    fn maybe_log_commit_stats
323:    fn maybe_log_render_stats
347:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/state/popups.rs</code> &mdash; 345 lines</summary>

```rust
1:impl MeridianShell
2:    fn toggle_launcher
70:    pub
110:    fn open_sound_settings_from_tray
133:    fn open_network_settings_from_tray
155:    pub
200:    pub
217:    pub
263:    pub
281:    pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/state/shell_actions.rs</code> &mdash; 289 lines</summary>

```rust
1:impl MeridianShell
2:    pub
15:    pub
72:    pub
97:    pub
109:    pub
126:    pub
131:    pub
153:    pub
173:    pub
186:    pub
200:    pub
212:    pub
243:    pub
257:    pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/state/timers.rs</code> &mdash; 465 lines</summary>

```rust
1:impl MeridianShell
2:    pub
10:    pub
18:    fn needs_fast_tick
26:    pub
35:    pub
156:    pub
165:    fn open_desktop_context_menu_from_ipc
202:    pub
208:    pub
224:    pub
242:    pub
260:    pub
270:    pub
285:    pub
309:    pub
323:    pub
341:    pub
360:    fn reassert_region_picker_layer_state
373:    pub
426:    pub
451:    fn close_desktop_context_menu_from_ipc
460:    fn desktop_menu_in_open_debounce
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/state_tests.rs</code> &mdash; 13 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/state_tests/output_events.rs</code> &mdash; 283 lines</summary>

```rust
2:fn output_workspace_changed_updates_known_output
36:fn output_workspace_changed_unknown_output_is_added_safely
65:fn output_workspace_changed_clamps_workspace_and_handles_focus_drop
98:fn output_workspace_snapshot_clamps_workspace_values
126:fn legacy_workspace_changed_still_works_with_and_without_output_aware_state
148:fn panel_active_workspace_prefers_focused_output_id
176:fn panel_active_workspace_falls_back_to_focused_flag
194:fn panel_active_workspace_falls_back_to_primary_output
222:fn panel_active_workspace_falls_back_to_first_output
250:fn panel_active_workspace_falls_back_to_legacy_when_unavailable
268:fn panel_active_workspace_normalizes_out_of_range
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/state_tests/workspaces.rs</code> &mdash; 347 lines</summary>

```rust
2:fn workspace_changed_clamps_workspace_range
11:fn panel_global_activation_point_offsets_bottom_panel_y
23:fn full_snapshot_recalculates_counts_and_active_workspace
67:fn empty_snapshot_marks_all_workspaces_empty
86:fn snapshot_with_one_window_marks_single_workspace_occupied
110:fn snapshot_workspace_values_out_of_range_are_clamped_safely
147:fn window_opened_updates_or_inserts_without_crash
165:fn window_closed_is_safe_for_unknown_id
178:fn window_closed_removes_existing_window
191:fn full_snapshot_preserves_workspace_on_window_entries
224:fn stale_focused_window_id_is_cleared_when_no_window_matches
238:fn resolve_shell_theme_from_config_applies_cursor_and_wallpaper_overrides
266:fn resolve_shell_theme_from_config_fails_for_unknown_theme
278:fn panel_theme_signature_changes_when_theme_changes
289:fn panel_theme_signature_changes_when_surface_alt_changes
300:fn panel_theme_signature_changes_when_border_changes
311:fn output_workspace_snapshot_with_two_outputs_is_stored
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/time.rs</code> &mdash; 50 lines</summary>

```rust
7:pub
13:pub
33:pub
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/types.rs</code> &mdash; 88 lines</summary>

```rust
2:pub
15:pub
24:pub enum ClickAction
42:impl ClickAction
43:    pub
67:pub struct ClickZone
74:pub struct Rect
81:impl Rect
82:    pub fn contains
```

</details>

<details><summary><code>crates/meridian-shell/src/widget_action.rs</code> &mdash; 445 lines</summary>

```rust
45:pub
109:pub
194:fn parse_default_apps_set_action
203:fn exact_action_for_id
240:fn settings_category_action_for_id
247:fn parse_indexed_action
255:fn parse_display_mode_select_action
269:    fn action_for_id_power_off
274:    fn action_for_id_power_logout
282:    fn action_for_id_settings_category
298:    fn action_for_id_indexed_ids
325:    fn action_for_id_cursor_size
335:    fn action_for_id_cursor_theme
350:    fn action_for_id_idle_timeout
364:    fn action_for_id_volume_and_mute
383:    fn action_for_id_default_audio_device
397:    fn action_for_id_activate_connection
407:    fn action_for_id_wifi_connect
417:    fn action_for_id_bluetooth
435:    fn action_for_id_indexed_ids_reject_malformed_suffixes
441:    fn action_for_id_unknown
```

</details>

<details><summary><code>crates/meridian-shell/src/widget_traversal.rs</code> &mdash; 64 lines</summary>

```rust
3:pub
29:    fn find_widget_at_path_empty_returns_root
38:    fn find_widget_at_path_out_of_bounds_returns_none
46:    fn find_widget_at_path_smoke_finds_apps_switch
```

</details>

<details><summary><code>crates/meridian-shell/src/wifi_password_modal.rs</code> &mdash; 315 lines</summary>

```rust
34:pub
42:fn button_rects
62:fn field_rect
72:pub
91:pub
207:fn fit
217:fn blit
262:    fn hit_button_distinguishes_connect_and_cancel
277:    fn hit_button_misses_outside_buttons
288:    fn buttons_and_field_lie_within_the_modal
299:    fn draw_does_not_panic_for_empty_and_long_input
```

</details>

<details><summary><code>crates/meridian-shell/src/workspaces.rs</code> &mdash; 177 lines</summary>

```rust
15:pub struct WorkspacePopupState
19:impl WorkspacePopupState
20:    pub fn new
25:pub struct WorkspacePopupInput
32:fn grid_geometry
45:pub fn workspace_popup_hover_idx
63:pub fn draw_workspace_popup
134:    fn workspace_popup_generates_nine_switch_click_zones
171:    fn workspace_popup_hover_idx_finds_first_tile_inside_grid
```

</details>

### `meridian-tokens`

<details><summary><code>crates/meridian-tokens/src/chrome.rs</code> &mdash; 109 lines</summary>

```rust
11:pub struct Scrollbar
18:impl Scrollbar
25:impl Default for Scrollbar
26:    fn default
33:pub struct Launcher
52:impl Launcher
65:impl Default for Launcher
66:    fn default
73:pub struct Mask
77:impl Mask
81:impl Default for Mask
82:    fn default
92:    fn defaults_locked
104:    fn defaults_via_default_trait_match_consts
```

</details>

<details><summary><code>crates/meridian-tokens/src/color.rs</code> &mdash; 330 lines</summary>

```rust
17:pub struct Color
24:impl Color
34:    pub fn as_f32_array
44:    pub fn to_hex
54:    pub fn lerp
70:impl FromStr for Color
71:    type Err = String;
73:    fn from_str
94:impl fmt::Display for Color
95:    fn fmt
101:    fn deserialize<D: serde::Deserializer<'de>>
111:pub struct Palette
125:impl Palette
191:pub fn relative_luminance
200:pub fn contrast_text
208:impl Default for Palette
210:    fn default
220:    fn rgb_sets_alpha_opaque
227:    fn rgba_preserves_alpha
232:    fn luminance_orders_dark_below_light
241:    fn contrast_text_picks_dark_on_light_and_white_on_dark
253:    fn lerp_midpoint_keeps_base_alpha
261:    fn from_str_parses_rgb_and_rgba
273:    fn from_str_rejects_multibyte_without_panicking
280:    fn from_str_rejects_wrong_length
286:    fn to_hex_roundtrips
292:    fn metro_palette_anchors_match_spec
301:    fn default_palette_is_dark
306:    fn dark_palette_anchors_match_mockup
315:    fn light_palette_anchors_match_mockup
324:    fn dark_and_light_differ_only_in_being_distinct_tables
```

</details>

<details><summary><code>crates/meridian-tokens/src/elevation.rs</code> &mdash; 84 lines</summary>

```rust
14:pub struct Elevation
23:impl Elevation
51:    fn levels_match_phase4_values
80:    fn launcher_sits_above_popup
```

</details>

<details><summary><code>crates/meridian-tokens/src/font.rs</code> &mdash; 9 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-tokens/src/interaction.rs</code> &mdash; 167 lines</summary>

```rust
18:pub struct Interaction
34:impl Interaction
45:    pub fn hover
50:    pub fn pressed
55:    pub fn accent_idle
60:    pub fn accent_hover
75:    pub fn selection
82:    pub fn armed
90:    pub fn selected_tint
96:    pub fn darken
101:impl Default for Interaction
102:    fn default
112:    fn hover_lightens_pressed_darkens
119:    fn accent_cushion_keeps_rgb_sets_alpha
128:    fn selection_blends_base_toward_accent
140:    fn armed_darkens_the_accent
147:    fn selected_tint_lightens_and_darken_darkens
156:    fn canonical_values_locked
```

</details>

<details><summary><code>crates/meridian-tokens/src/lib.rs</code> &mdash; 23 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-tokens/src/radius.rs</code> &mdash; 87 lines</summary>

```rust
10:pub struct Radius
18:impl Radius
46:impl Default for Radius
47:    fn default
57:    fn metro_radius_is_all_zero
67:    fn default_scale_matches_phase1_values
79:    fn scales_are_non_decreasing
```

</details>

### `meridian-ui`

<details><summary><code>crates/meridian-ui/src/effect/border.rs</code> &mdash; 86 lines</summary>

```rust
8:pub fn paint_border
26:fn to_tiny_skia_color
41:    fn border_paint_changes_pixels
69:    fn empty_like_path_is_noop
```

</details>

<details><summary><code>crates/meridian-ui/src/effect/dominant_color.rs</code> &mdash; 115 lines</summary>

```rust
9:pub fn dominant_color
65:    fn solid_pixmap
79:    fn solid_red_returns_red
88:    fn solid_blue_returns_blue
96:    fn grayscale_returns_fallback
103:    fn transparent_returns_fallback
110:    fn near_black_returns_fallback
```

</details>

<details><summary><code>crates/meridian-ui/src/effect/fill.rs</code> &mdash; 75 lines</summary>

```rust
8:pub fn paint_fill
21:fn to_tiny_skia_color
36:    fn fill_paint_changes_pixels
59:    fn empty_like_path_is_noop
```

</details>

<details><summary><code>crates/meridian-ui/src/effect/metro_surface.rs</code> &mdash; 77 lines</summary>

```rust
14:pub fn paint_metro_surface
53:    fn paint_metro_surface_draws_body_and_stripe
```

</details>

<details><summary><code>crates/meridian-ui/src/effect/mod.rs</code> &mdash; 24 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-ui/src/effect/radius.rs</code> &mdash; 119 lines</summary>

```rust
9:pub fn rounded_rect_path
50:    fn assert_bounds_eq
59:    fn radius_zero_matches_rect_bounds
71:    fn positive_radius_produces_non_empty_path
84:    fn radius_is_clamped_to_half_extent
97:    fn degenerate_rect_returns_none
```

</details>

<details><summary><code>crates/meridian-ui/src/effect/text.rs</code> &mdash; 367 lines</summary>

```rust
22:fn srgb_to_linear
31:fn linear_to_srgb
42:pub struct TextInk
47:impl TextInk
48:    pub fn new
66:pub fn blend_text_sample
97:fn freetype_font
101:fn replace_freetype_font
107:fn embedded_ui_font
117:pub fn ui_font
128:pub fn set_ui_font
142:pub fn clear_ui_font
147:pub fn measure_text
177:pub fn ui_line_metrics
184:pub fn paint_text
238:fn paint_text_freetype
292:pub fn truncate_to_fit
329:    fn font_parses_with_line_metrics
335:    fn measure_text_returns_positive_for_non_empty
342:    fn measure_text_empty_is_zero
349:    fn paint_text_writes_pixels
360:    fn paint_text_empty_string_is_noop
```

</details>

<details><summary><code>crates/meridian-ui/src/event/hit_test.rs</code> &mdash; 315 lines</summary>

```rust
5:pub fn hit_test
12:fn hit_test_node
51:    fn hit_test_returns_none_outside_root
75:    fn hit_test_returns_empty_path_for_root_only_hit
99:    fn hit_test_picks_correct_sibling
143:    fn hit_test_picks_deepest_child
191:    fn hit_test_accumulates_parent_offsets
269:    fn widget_path_iter_yields_indices_in_order
276:    fn widget_path_empty_is_empty
287:    fn pointer_button_eq_per_variant
298:    fn event_pointer_press_carries_position_and_button
```

</details>

<details><summary><code>crates/meridian-ui/src/event/mod.rs</code> &mdash; 76 lines</summary>

```rust
6:pub enum WidgetState
14:pub struct PointerPosition
20:pub enum PointerButton
27:pub enum Event
46:pub struct WidgetPath
50:impl WidgetPath
51:    pub fn empty
57:    pub fn from_vec
61:    pub fn iter
65:    pub fn len
69:    pub fn is_empty
73:    pub fn as_slice
```

</details>

<details><summary><code>crates/meridian-ui/src/lib.rs</code> &mdash; 75 lines</summary>

```rust
38:    fn taffy_computes_basic_flex_layout
70:    fn tiny_skia_pixmap_allocates
```

</details>

<details><summary><code>crates/meridian-ui/src/paint/layout.rs</code> &mdash; 163 lines</summary>

```rust
12:pub struct LayoutNode
18:pub struct LayoutTree
22:struct PendingNode
31:pub fn compute_layout
48:fn build_taffy_subtree
69:fn extract_layout_subtree
97:    fn computes_row_layout_for_two_fixed_children
144:    fn root_matches_requested_size
```

</details>

<details><summary><code>crates/meridian-ui/src/paint/mod.rs</code> &mdash; 26 lines</summary>

```rust
14:pub struct Rect
23:pub struct PixelSize
```

</details>

<details><summary><code>crates/meridian-ui/src/paint/render.rs</code> &mdash; 455 lines</summary>

```rust
8:pub enum RenderError
12:pub fn render
23:pub fn render_idle
33:fn render_node
92:    fn render_smoke_does_not_crash
126:    fn render_accumulates_offset_across_nested_containers
163:    fn widget_state_default_is_idle
168:    fn widget_state_eq_per_variant
179:    fn tile_paint_hovered_differs_from_idle
225:    fn tile_paint_pressed_differs_from_idle
271:    fn button_paint_hovered_differs_from_idle
310:    fn render_node_path_root_is_empty
350:    fn render_node_path_indices_for_child
388:    fn render_with_idle_state_matches_legacy
441:    fn lerp_color_endpoints
449:    fn lerp_color_preserves_alpha
```

</details>

<details><summary><code>crates/meridian-ui/src/style/color.rs</code> &mdash; 6 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-ui/src/style/mod.rs</code> &mdash; 52 lines</summary>

```rust
16:pub struct Theme
22:impl Theme
36:    fn default_theme_bundles_metro_tokens
44:    fn token_types_are_copy
45:        fn assert_copy<T: Copy>
```

</details>

<details><summary><code>crates/meridian-ui/src/style/radius.rs</code> &mdash; 6 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-ui/src/style/spacing.rs</code> &mdash; 40 lines</summary>

```rust
4:pub struct Spacing
12:impl Spacing
27:    fn default_spacing_is_strictly_monotonic
36:    fn default_spacing_is_positive
```

</details>

<details><summary><code>crates/meridian-ui/src/widget/base.rs</code> &mdash; 540 lines</summary>

```rust
12:pub trait Widget
14:    fn style
19:    fn paint
24:    fn id
32:    fn launch_info
41:    fn launch_exec
48:    fn focus_window_id
55:    fn pinned_app_idx
60:    fn children
66:pub struct Container
71:impl Container
72:    pub fn new
76:    pub fn leaf
80:    pub fn centered_viewport
97:    pub fn top_viewport
130:    pub fn flow
157:    pub fn column
172:    pub fn row
187:    pub fn grid
220:    pub fn footer_row
256:    fn horizontal_cluster
273:impl Widget for Container
274:    fn style
278:    fn paint
281:    fn children
296:    fn container_leaf_has_no_children
302:    fn container_style_roundtrips
316:    fn centered_viewport_sets_center_alignment_and_size
329:    fn flow_wraps_mixed_child_sizes
380:    fn grid_places_tiles_on_cell_boundaries
454:    fn column_stacks_children_vertically_with_gap
487:    fn container_row_has_correct_child_count
509:    fn footer_row_places_left_and_right_clusters
```

</details>

<details><summary><code>crates/meridian-ui/src/widget/button.rs</code> &mdash; 352 lines</summary>

```rust
24:pub struct Button
37:impl Button
38:    pub fn new
51:    pub fn with_id
70:    pub fn with_id_and_icon
93:    pub fn with_armed_progress
99:    pub fn with_armed_label
104:    pub fn label
108:    pub fn accent
112:    pub fn width
116:    pub fn height
121:impl Widget for Button
122:    fn id
126:    fn style
136:    fn paint
201:fn paint_progress_ring
250:    fn button_new_stores_fields
259:    fn button_style_uses_explicit_size
278:    fn button_paint_smoke
298:    fn button_with_id_and_icon_none_does_not_panic
323:    fn armed_icon_button_draws_confirmation_label
```

</details>

<details><summary><code>crates/meridian-ui/src/widget/mod.rs</code> &mdash; 14 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-ui/src/widget/tile.rs</code> &mdash; 333 lines</summary>

```rust
38:pub enum TileSize
45:impl TileSize
46:    pub fn dimensions
55:    pub fn cell_span
65:pub struct Tile
74:impl Tile
75:    pub fn new
86:    pub fn with_id
97:    pub fn with_exec_and_icon
114:    pub fn label
118:    pub fn accent
122:    pub fn size
127:impl Widget for Tile
128:    fn id
132:    fn launch_exec
136:    fn style
145:    fn paint
196:    fn tile_size_dimensions_match_win10_scale
216:    fn tile_size_cell_span_matches_win10_scale
224:    fn tile_new_stores_label_accent_and_size
236:    fn tile_style_forwards_cell_spans
258:    fn tile_paint_draws_stripe_and_body_for_wide_tile
292:    fn tile_with_exec_stores_exec
304:    fn tile_new_launch_exec_is_none
310:    fn tile_paint_with_none_icon_does_not_panic
```

</details>

### `meridian-wm`

<details><summary><code>crates/meridian-wm/src/floating.rs</code> &mdash; 2 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-wm/src/lib.rs</code> &mdash; 7 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-wm/src/tiling/layout.rs</code> &mdash; 90 lines</summary>

```rust
13:pub struct TilingLayout
18:impl TilingLayout
19:    pub fn new
26:    pub fn is_empty
30:    pub fn windows
38:    pub fn add
54:    pub fn remove
61:    pub fn compute_rects
78:    pub fn adjust_split
86:impl Default for TilingLayout
87:    fn default
```

</details>

<details><summary><code>crates/meridian-wm/src/tiling/mod.rs</code> &mdash; 6 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-wm/src/tiling/tree.rs</code> &mdash; 454 lines</summary>

```rust
5:pub
15:pub
25:pub
37:fn split_rect
70:pub
93:pub
126:pub
141:pub
163:pub
227:pub
270:    fn assert_positive_sizes
281:    fn split_rect_horizontal_applies_gap_and_ratio
297:    fn split_rect_vertical_applies_gap_and_ratio
313:    fn split_rect_horizontal_clamps_extreme_ratios
328:    fn split_rect_vertical_clamps_extreme_ratios
343:    fn split_rect_horizontal_width_zero_keeps_sizes_positive
350:    fn split_rect_horizontal_width_one_keeps_sizes_positive
357:    fn split_rect_vertical_height_zero_keeps_sizes_positive
364:    fn split_rect_vertical_height_one_keeps_sizes_positive
371:    fn insert_at_last_keeps_deterministic_in_order
382:    fn insert_next_to_inserts_beside_focused_leaf
396:    fn remove_from_node_collapses_parent_when_child_removed
416:    fn contains_window_detects_existing_and_missing_values
428:    fn insert_unique_skips_duplicate_window
442:    fn insert_unique_inserts_new_window
```

</details>

<details><summary><code>crates/meridian-wm/src/tiling/types.rs</code> &mdash; 29 lines</summary>

```rust
2:pub enum SplitDir
7:impl SplitDir
8:    pub fn other
21:    fn other_flips_horizontal_to_vertical
26:    fn other_flips_vertical_to_horizontal
```

</details>

<details><summary><code>crates/meridian-wm/src/window.rs</code> &mdash; 1 lines</summary>

```rust
```

</details>

<details><summary><code>crates/meridian-wm/src/workspace.rs</code> &mdash; 159 lines</summary>

```rust
9:pub enum WorkspaceMode
14:pub struct WmWorkspace
22:impl WmWorkspace
23:    pub fn new
33:    pub fn toggle_mode
45:    pub fn add_tiled
53:    pub fn remove_window
59:    pub fn tiled_windows
64:    pub fn remove_tiled
70:    pub fn set_floating
83:    pub fn is_floating
89:    pub fn rebuild_tiling_from
102:    pub fn compute_tiled
118:    pub fn resize_focused
122:    pub fn force_split
127:impl Default for WmWorkspace
128:    fn default
139:    fn new_sets_expected_defaults
148:    fn toggle_mode_switches_between_floating_and_tiling
```

</details>

### `meridian (root binary)`

<details><summary><code>src/main.rs</code> &mdash; 232 lines</summary>

```rust
26:struct ShellWatchdog
36:impl ShellWatchdog
37:    fn new
51:    fn start
98:    fn watch
131:    fn bump_restart_delay
135:    fn stop
145:impl Drop for ShellWatchdog
146:    fn drop
151:fn find_shell_binary
163:fn next_restart_delay
168:fn env_flag_enabled
179:fn main
```

</details>


<!-- END GENERATED -->
