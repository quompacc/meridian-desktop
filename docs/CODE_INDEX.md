# Meridian — Code Index

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
- Per-tick work (clock, battery, **audio re-poll**, **launcher refresh swap-in**): `wayland/state.rs::tick`.
- Shell state struct (the ~205-field god-object): `wayland/state.rs` → `wayland/shell.rs` (`struct MeridianShell`).
- Shell construction / startup polls: `wayland/init.rs`.

### Panel
- Render: `wayland/render.rs::draw_panel`; signature/dedup: `wayland/render.rs` (`panel_render_signature`).
- Widget tree: `panel_view.rs::build_panel_widget_tree`.
- Click handling: `wayland/state.rs::handle_panel_click`.

### Launcher  *(LAUNCH-2 fix)*
- State + desktop-entry scan: `launcher.rs` (`LauncherState`, `DesktopApp::load_system`).
- Open/close + async refresh trigger: `wayland/state.rs::toggle_launcher` → `request_launcher_apps_refresh`.
- Background rescan swap-in: `wayland/state.rs::poll_launcher_apps_refresh` (called from `tick`).
- Render: `wayland/render.rs::draw_launcher`.

### Settings (the 4100-line file)
- View builders / row widgets: `settings_view.rs` (`build_settings_widget_tree`, ~28 `impl Widget`).
- Action dispatch: `wayland/handlers/widget_dispatch.rs::dispatch_widget_action` → `dispatch_settings_action`.

### Audio  *(AUDIO-1 fix)*
- Snapshot model + `is_settled`: `audio/mod.rs` (`AudioSnapshot`).
- Linux backend (PipeWire via `wpctl`): `audio/wpctl.rs::snapshot` / `set_volume` / `toggle_mute`.
- FreeBSD backend (`mixer(8)`): `audio/mixer.rs` (cfg-gated, not built on Linux).
- Tray popup: `audio_popup.rs`; popup open/re-poll: `wayland/state.rs::toggle_audio_popup`.

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

## Big files (refactor candidates)

| File | Lines | Note |
|---|---|---|
| `meridian-shell/src/settings_view.rs` | ~4100 | 28 row widgets + one giant per-category builder. |
| `meridian-shell/src/wayland/state.rs` | ~3000 | One `impl MeridianShell` — lifecycle + ~70 methods. |
| `meridian-login/src/main.rs` | ~2800 | DRM + UI state + auth + theme + draw + anim in one bin. |
| `meridian-compositor/src/protocols/xwayland.rs` | ~2000 | Helpers + `XwmHandler` impl. |
| `meridian-config/src/config.rs` | ~2000 | **~280 code + ~1700 tests** — not actually a big code file. |
| `meridian-shell/src/wayland/render.rs` | ~1900 | ~40 `draw_*` methods. |

Full split proposals: [`REFACTORING_PLAN.md`](REFACTORING_PLAN.md).

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

<details><summary><code>crates/meridian-compass-render/src/lib.rs</code> &mdash; 1333 lines</summary>

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
405:pub fn needle_angle_deg
420:fn color_with_alpha
429:fn draw_background
474:fn draw_compass_shadow
522:fn draw_meridian_lines
544:fn draw_scale_ring
652:fn draw_sweep_glint
690:fn draw_rose_shadow
734:fn draw_rose
795:fn draw_needle
878:fn draw_needle_glow
915:fn draw_pivot
956:fn draw_signature
962:fn draw_cardinals
981:fn draw_heading_mark
1004:struct TextMetrics
1010:fn measure_text
1038:fn draw_text_at_baseline
1082:fn draw_text_centered
1103:    fn quompacc_fonts_construct_a_painter
1109:    fn garbage_sans_font_yields_build_error
1121:    fn garbage_script_font_yields_build_error
1133:    fn needle_angle_finite_for_relevant_t_range
1141:    fn needle_angle_at_settle_is_near_north
1148:    fn renders_without_panic_at_typical_resolutions
1171:    fn veil_alpha_255_yields_black_frame
1194:    fn north_glow_position_consistent_with_needle_angle
1214:    fn glow_base_radius_matches_internal_geometry
1222:    fn render_glow_at_alone_lights_up_pixels
1239:    fn measure_text_width_is_positive_for_non_empty
1250:    fn render_text_left_returns_pen_past_last_glyph
1266:    fn watermark_alpha_dims_compass_toward_background
1299:    fn north_glow_disabled_changes_some_pixels
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

<details><summary><code>crates/meridian-compositor/src/backend/drm/gpu.rs</code> &mdash; 118 lines</summary>

```rust
20:pub
54:fn probe_gpu_connectors
80:fn probe_connected
81:    struct ProbeDrmDevice<'a>
82:    impl AsFd for ProbeDrmDevice<'_>
83:        fn as_fd
87:    impl smithay::reexports::drm::Device for ProbeDrmDevice<'_>
88:    impl smithay::reexports::drm::control::Device for ProbeDrmDevice<'_>
102:pub
```

</details>

<details><summary><code>crates/meridian-compositor/src/backend/drm/init.rs</code> &mdash; 1417 lines</summary>

```rust
69:struct DrmConnectorReconfigureCandidate
79:struct DrmConnectorChangeSet
86:struct DrmConnectorRemoveCandidate
92:struct PendingInitOutput
106:pub
118:pub
176:fn sync_primary_flags_from_resolved_layout
212:fn parse_output_scale
225:fn output_scale_from_value
233:fn classify_drm_connector_changes
274:fn scan_drm_connectors_for_h5b
440:fn add_drm_output_via_hotplug_pipeline
728:fn remove_drm_output_via_hotplug_pipeline
805:fn detach_drm_output
820:fn configure_repaint_interval
847:fn register_drm_event_source<Source>
898:fn register_repaint_timer_source
945:fn register_libinput_event_source
957:pub fn init_drm
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

<details><summary><code>crates/meridian-compositor/src/backend/drm/mod.rs</code> &mdash; 696 lines</summary>

```rust
35:pub type GbmDrmCompositor =
39:pub
59:struct DurationStats
66:impl DurationStats
67:    fn record
80:    fn avg_ms
87:    fn min_ms
94:    fn max_ms
103:pub struct DrmTimingStats
138:pub struct PerOutputDirtyStats
148:pub struct DrmDirtyStats
157:impl DrmDirtyStats
158:    pub fn new
173:    pub fn register_output
181:    pub fn unregister_output
189:    pub fn record_dirty_mark_event
197:    pub fn record_dirty_set
207:    pub fn record_dirty_clear
217:    pub fn record_skipped_clean
227:    pub fn record_skipped_power_off
237:    pub fn record_rendered_dirty
247:    pub fn record_rendered_while_not_dirty
257:    pub fn report_if_due
307:impl DrmTimingStats
308:    pub fn new
355:    pub
408:    pub
435:    fn report_if_due
531:pub struct DrmOutput
552:pub struct DisabledDrmOutput
560:pub enum DrmCursorIcon
568:pub struct DrmBackend
587:impl DrmBackend
588:    pub fn disable_output
625:    pub fn enable_output_pull_pending
633:    pub fn rebuild_compositor_for_mode
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

<details><summary><code>crates/meridian-compositor/src/backend/drm/render.rs</code> &mdash; 1624 lines</summary>

```rust
58:fn themed_layer_glass_info
87:fn render_scene_for_blur
137:fn is_glass_blur_source_excluded
148:fn first_rendered_blur_source
159:struct PendingGlassBatch
165:fn blur_pass
229:fn blur_scene
249:fn clear_output_dirty
266:fn render_window_popup_elements<C>
303:fn render_window_toplevel_elements<C>
370:pub
1130:fn serve_screencopy_frames
1220:fn process_thumbnail_requests
1367:fn process_screenshot_requests
1505:fn screenshot_png_bytes
1533:fn encode_screenshot_png
1550:fn crop_xrgb
1563:fn sanitize_window_id
1575:fn scale_down_xrgb
1596:    fn encodes_xrgb_with_red_blue_swapped
1608:    fn dimensions_and_blue_channel_round_trip
1621:    fn short_buffer_is_an_error_not_a_panic
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

<details><summary><code>crates/meridian-compositor/src/cursor/image.rs</code> &mdash; 243 lines</summary>

```rust
18:pub struct CursorImage
28:impl CursorImage
29:    pub fn load_default
43:    pub fn load_theme
48:    pub fn load_theme_icon
118:    pub fn load_theme_cursor_icon
137:    pub fn embedded
141:    pub fn embedded_sized
145:    fn embedded_sized_with_kind
162:    pub fn is_valid_visible_image
183:    pub fn to_memory_buffer
199:fn embedded_kind_for_icon_names
235:fn embedded_name_for_icon_names
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

<details><summary><code>crates/meridian-compositor/src/decoration/render/elements.rs</code> &mdash; 830 lines</summary>

```rust
83:fn shadow_uniform_names
147:fn rounded_quad_uniform_names
159:fn rounded_quad_element
243:fn glass_uniform_names
256:fn glass_titlebar_element
278:impl DecorationManager
280:    pub fn render_elements
826:impl From<SolidColorRenderElement> for DecorationRenderElement
827:    fn from
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

<details><summary><code>crates/meridian-compositor/src/grabs/move_grab.rs</code> &mdash; 1215 lines</summary>

```rust
28:fn is_pointer_near_output_top_edge
36:fn is_pointer_near_output_left_edge
44:fn is_pointer_near_output_right_edge
54:enum MoveReleaseEdgeAction
59:fn release_edge_action_for_output
75:fn select_move_release_output
82:fn move_release_workarea_geometry
86:fn release_edge_action_on_move_release
97:fn should_maximize_on_move_release
133:fn maximize_window_from_move_release
139:fn apply_half_snap_tiled_states
156:fn apply_half_snap_from_move_release
206:fn select_output_geometry_for_rect_center
217:fn rect_matches_output_fullscreen_shape
227:fn window_is_output_fullscreen_shape
236:fn xwayland_snap_rect_for_action
262:fn apply_xwayland_snap_from_move_release
339:fn half_snap_restore_geometry_source
354:fn movement_crosses_restore_threshold
363:fn restored_initial_window_location
372:fn pointer_ratio_within_frame_x
380:struct DragRestorePointerAnchor
385:fn frame_geometry_from_client
397:fn drag_restore_anchor_from_start_pointer
419:fn anchored_client_location_from_pointer
435:fn anchored_restore_client_location
450:fn maybe_restore_maximized_drag
494:fn window_half_snap_direction
508:fn consume_half_snap_restore_geometry
519:fn apply_half_snap_drag_restore_states
528:fn maybe_restore_half_snapped_drag
579:fn xwayland_restore_window_key
587:fn maybe_restore_xwayland_snapped_drag
630:pub struct MoveSurfaceGrab
644:impl PointerGrab<MeridianState> for MoveSurfaceGrab
645:    fn motion
730:    fn relative_motion
740:    fn button
783:    fn axis
792:    fn frame
800:    fn gesture_swipe_begin
808:    fn gesture_swipe_update
816:    fn gesture_swipe_end
824:    fn gesture_pinch_begin
832:    fn gesture_pinch_update
840:    fn gesture_pinch_end
848:    fn gesture_hold_begin
856:    fn gesture_hold_end
865:    fn start_data
869:    fn unset
891:    fn point
896:    fn top_edge_threshold_detects_near_top
908:    fn top_edge_threshold_rejects_deeper_positions
920:    fn top_edge_threshold_requires_pointer_inside_output
932:    fn drag_restore_threshold_requires_real_drag_distance
942:    fn anchored_restore_location_preserves_pointer_horizontal_ratio
961:    fn drag_restore_anchor_clamps_pointer_ratio_near_left_edge
978:    fn drag_restore_anchor_clamps_pointer_ratio_near_right_edge
995:    fn anchored_restore_location_applies_floating_insets_after_frame_anchor
1016:    fn left_edge_release_triggers_left_half_snap
1032:    fn right_edge_release_triggers_right_half_snap
1048:    fn top_edge_maximize_precedes_side_snap
1061:    fn release_away_from_edges_does_not_snap
1076:    fn move_release_workarea_subtracts_panel_reservation
1091:    fn half_snap_tiled_states_are_set_for_left_and_right
1108:    fn half_snap_restore_prefers_maximize_restore_geometry
1128:    fn consume_half_snap_restore_geometry_prefers_and_consumes_stored_entry
1148:    fn consume_half_snap_restore_geometry_falls_back_to_current_geometry
1162:    fn half_snap_drag_restore_clears_tiled_bits_and_preserves_other_states
1182:    fn xwayland_snap_rect_for_maximize_uses_workarea_geometry
1195:    fn xwayland_snap_rect_for_half_snap_splits_width
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

<details><summary><code>crates/meridian-compositor/src/input/pointer/button.rs</code> &mdash; 887 lines</summary>

```rust
32:fn select_pointer_button_output_info
56:fn surface_belongs_to_layer
71:fn xwayland_override_redirect_window_under_pointer
93:fn started_move_grab_window_states
109:fn decoration_resize_edge_to_resize_edge
122:fn raise_window_and_focus
138:pub fn handle_pointer_button<I: InputBackend>
176:        type HitInfo =
746:    fn click_point_on_output_one
784:    fn click_point_on_output_two
822:    fn outside_point_uses_primary_fallback
844:    fn no_primary_uses_first_fallback
882:    fn empty_registry_is_safe
```

</details>

<details><summary><code>crates/meridian-compositor/src/input/pointer/mod.rs</code> &mdash; 690 lines</summary>

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
500:    fn reg
516:    fn absolute_point_selects_output_one
527:    fn absolute_point_selects_output_two
538:    fn resize_hit_maps_to_expected_cursor_icons
590:    fn non_resize_hit_maps_to_default_cursor_icon
602:    fn absolute_point_outside_uses_primary_fallback
613:    fn focus_update_candidate_is_none_outside_outputs
621:    fn relative_clamp_keeps_point_inside_bounds
631:    fn relative_clamp_noop_when_inside_bounds
640:    fn xwayland_edge_hit_detects_corners_and_edges
678:    fn xwayland_edge_hit_ignores_interior_and_outside_points
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

<details><summary><code>crates/meridian-compositor/src/protocols/xwayland.rs</code> &mdash; 2018 lines</summary>

```rust
41:fn select_output_geometry_for_rect
53:fn rect_matches_output_fullscreen_shape
63:fn panel_safe_normal_xwayland_rect
89:fn configure_request_rect
112:fn adjusted_configure_request_rect
126:pub
133:pub
159:fn find_active_x11_window
169:fn find_x11_window_with_workspace
181:fn find_x11_surface_by_window_id
194:fn find_x11_window_by_stacking_id
214:fn restack_override_redirect_above_hint
253:fn reorder_above_hint
260:fn update_or_diag_entry<F>
269:fn window_is_output_fullscreen_shape
279:fn x11_resize_edge_to_resize_edge
292:pub
296:fn x11_fullscreen_restore_key
300:fn maximized_x11_content_size
315:trait DecorationSyncTarget
316:    fn set_ssd
317:    fn set_focused
318:    fn set_maximized
319:    fn set_fullscreen
322:struct SurfaceDecorationSyncTarget<'a>
327:impl DecorationSyncTarget for SurfaceDecorationSyncTarget<'_>
328:    fn set_ssd
332:    fn set_focused
337:    fn set_maximized
342:    fn set_fullscreen
348:fn apply_managed_map_ssd
359:fn apply_override_redirect_ssd
363:pub
381:pub
441:struct X11Remeasure
453:pub
518:pub
554:pub fn start_xwayland
600:impl XWaylandShellHandler for MeridianState
601:    fn xwayland_shell_state
605:    fn surface_associated
633:impl XwmHandler for MeridianState
634:    fn xwm_state
640:    fn new_window
650:    fn new_override_redirect_window
709:    fn map_window_request
781:    fn map_window_notify
783:    fn mapped_override_redirect_window
838:    fn unmapped_window
901:    fn destroyed_window
949:    fn configure_request
1155:    fn configure_notify
1247:    fn maximize_request
1251:    fn unmaximize_request
1255:    fn fullscreen_request
1304:    fn unfullscreen_request
1342:    fn minimize_request
1424:    fn unminimize_request
1483:    fn property_notify
1489:    fn resize_request
1585:    fn move_request
1665:    fn allow_selection_access
1689:    fn new_selection
1700:    fn cleared_selection
1715:    fn send_selection
1742:    fn disconnected
1761:    struct MockDecorationSyncTarget
1768:    impl DecorationSyncTarget for MockDecorationSyncTarget
1769:        fn set_ssd
1773:        fn set_focused
1777:        fn set_maximized
1781:        fn set_fullscreen
1787:    fn normal_xwayland_rect_is_clamped_to_panel_safe_bottom
1805:    fn output_sized_rect_is_treated_as_fullscreen_and_left_unchanged
1823:    fn configure_request_rect_uses_requested_x_y_when_present
1833:    fn override_redirect_configure_bypasses_panel_clamp
1846:    fn managed_configure_still_clamps_to_panel_safe_workarea
1863:    fn override_redirect_always_passes_through
1878:    fn managed_normal_window_passes_through
1889:    fn managed_maximized_request_matching_workarea_is_deny_noop
1899:    fn managed_maximized_request_with_other_rect_is_implicit_unmaximize
1910:    fn managed_maximized_request_with_same_size_but_other_loc_is_implicit_unmaximize
1928:    fn managed_fullscreen_request_matching_output_is_deny_noop
1938:    fn managed_fullscreen_request_with_other_rect_is_implicit_unfullscreen
1949:    fn managed_fullscreen_takes_priority_over_maximized
1960:    fn decoration_state_sync_after_map_managed
1970:    fn decoration_state_sync_for_override_redirect_is_no_ssd
1983:    fn apply_managed_map_ssd_is_idempotent_when_called_twice
1994:    fn apply_override_redirect_ssd_overrides_prior_managed_state
2002:    fn maximized_x11_content_size_subtracts_frame_insets_in_both_axes
2008:    fn maximized_x11_content_size_clamps_minimum_dimension_to_one
2014:    fn maximized_x11_content_size_with_zero_decoration_offset_equals_workarea
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

<details><summary><code>crates/meridian-compositor/src/state/handlers/misc.rs</code> &mdash; 344 lines</summary>

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
154:impl OutputHandler for MeridianState
156:impl SelectionHandler for MeridianState
157:    type SelectionUserData =
159:    fn new_selection
176:    fn send_selection
196:impl PrimarySelectionHandler for MeridianState
197:    fn primary_selection_state
202:impl WaylandDndGrabHandler for MeridianState
204:impl DataDeviceHandler for MeridianState
205:    fn data_device_state
210:impl DndGrabHandler for MeridianState
212:impl XdgDecorationHandler for MeridianState
213:    fn new_decoration
231:    fn request_mode
246:    fn unset_mode
273:    fn moves_client_down_when_frame_top_would_be_offscreen
282:    fn moves_client_right_when_left_border_would_be_offscreen
292:    fn keeps_location_when_frame_is_already_fully_visible
301:    fn oversized_window_keeps_top_left_reachable
310:impl MeridianState
311:    pub fn update_focus_decoration
320:    pub fn set_keyboard_focus_with_decorations
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/handlers/mod.rs</code> &mdash; 10 lines</summary>

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

<details><summary><code>crates/meridian-compositor/src/state/ipc/server.rs</code> &mdash; 697 lines</summary>

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
487:    fn same_uid_is_allowed
492:    fn different_uid_is_rejected
512:    fn peer_uid_of_socketpair_matches_own_euid
520:    fn env_lock
525:    fn temp_runtime_dir
538:    fn with_runtime_dir<R>
550:    fn connect_client
554:    fn write_command
560:    fn unauthenticated_control_command_is_ignored
575:    fn authenticated_shell_control_command_is_accepted
597:    fn broadcasts_only_reach_authenticated_shell_clients
639:    fn cleanup_check_matches_original_socket_identity
664:    fn cleanup_check_rejects_replaced_non_socket_path
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

<details><summary><code>crates/meridian-compositor/src/state/mod.rs</code> &mdash; 741 lines</summary>

```rust
87:pub struct MaximizeRestoreGeometry
92:impl MaximizeRestoreGeometry
93:    pub fn new
102:pub enum HalfSnapDirection
108:pub enum WindowSnapState
113:pub struct HalfSnapRestoreGeometry
118:impl HalfSnapRestoreGeometry
119:    pub fn new
128:pub struct HalfSnapPlacement
134:pub struct MinimizedWindowEntry
141:pub struct XwaylandOrDiagConfigureRequest
154:pub struct XwaylandOrDiagConfigureNotify
161:pub struct XwaylandOrDiagPointerEvent
170:pub struct XwaylandOrDiagReleaseCandidate
180:pub struct XwaylandOrDiagReleaseState
194:pub struct XwaylandOrDiagEntry
214:pub
222:pub
229:pub
236:pub
254:pub
265:pub
280:pub
307:pub
319:pub
327:pub struct ThumbnailRequest
336:pub struct PendingScreenshotRequest
341:pub struct MeridianState
425:impl MeridianState
426:    pub fn resolve_output_layout
432:    pub
455:    pub fn clear_window_runtime_state
462:    pub fn keyboard_focus_diag_target
516:    fn capture_known_loc_and_size_preserves_client_size
529:    fn premaximized_entry_allows_missing_client_size
541:    fn existing_entry_is_not_overwritten
552:    fn missing_restore_entry_uses_fallback_location
559:    fn maximize_mapping_adds_decoration_offset_to_output_origin
566:    fn normal_window_workarea_subtracts_bottom_panel_reservation
584:    fn normal_window_workarea_rect_preserves_origin_and_width
596:    fn unmaximize_restore_uses_stored_geometry_without_fallback
607:    fn unmaximize_restore_uses_decoration_offset_when_missing
614:    fn clear_tiled_toplevel_states_unsets_only_tiled_bits
634:    fn half_snap_left_placement_uses_left_output_half
652:    fn half_snap_right_placement_uses_right_output_half
670:    fn half_snap_nonzero_output_origin_is_preserved
688:    fn half_snap_placement_applies_ssd_offset_and_inset
715:    fn half_snap_odd_output_width_assigns_extra_pixel_to_right_half
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_hotplug_tests.rs</code> &mdash; 1242 lines</summary>

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
541:fn single_output_add_yields_zero_origin_primary
551:fn two_outputs_default_layout_chains_horizontally
563:fn remove_primary_falls_back_to_remaining
575:fn add_remove_add_is_idempotent
589:fn remove_unknown_output_is_safe_noop
600:fn reconfigure_changes_width_keeps_chain_after
613:fn reconfigure_unknown_output_is_safe_noop
620:fn reconfigure_same_size_is_stable
629:fn layout_with_explicit_primary_overrides_first_default
642:fn layout_right_of_chain_three_outputs
670:fn layout_below_chain_two_outputs
688:fn layout_coord_position_pins_exact_xy
706:fn layout_dangling_reference_falls_back_to_auto
724:fn safety_net_triggers_when_all_outputs_disabled_in_layout
740:fn cyclic_add_remove_stability
759:fn four_outputs_complex_layout_snapshot
795:fn reload_layout_changes_primary_assignment
813:fn reload_layout_repositions_output_via_below
831:fn reload_layout_coord_pin_overrides_chain
849:fn reload_layout_empty_entries_falls_back_to_auto_chain
868:fn reload_layout_safety_net_re_enables_all_disabled
884:fn reload_layout_noop_when_registry_empty
896:fn reload_layout_with_mode_change_safely_logs_only_in_harness
918:fn reload_with_enabled_false_simulates_disable_via_registry
935:fn reload_re_enabling_output_re_adds_to_registry
959:fn reload_safety_net_re_enables_all_disabled_via_harness
975:fn window_on_removed_output_stays_at_logical_position
992:fn window_on_remaining_output_undisturbed_by_sibling_removal
1008:fn window_straddling_two_outputs_loses_one_on_removal
1024:fn reconfigure_geometry_keeps_window_position_updates_overlap
1040:fn cyclic_add_remove_add_does_not_leak_entered_outputs
1058:fn multiple_windows_distributed_across_outputs_snapshot
1078:fn multi_workspace_window_survives_output_remove
1100:fn reload_position_change_to_below_window_loses_overlap
1122:fn reload_position_change_brings_output_under_window
1144:fn disable_then_re_enable_output_window_overlap_restored
1175:fn window_moved_between_workspaces_preserves_overlap
1195:fn reconfigure_grow_brings_window_into_output
1211:fn safety_net_re_enable_keeps_window_overlap
1231:fn entry_with
```

</details>

<details><summary><code>crates/meridian-compositor/src/state/output_layout.rs</code> &mdash; 997 lines</summary>

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
311:    fn empty_layout_two_outputs_chains_horizontally
330:    fn coord_position_places_at_exact_xy
353:    fn right_of_resolves_to_target_right_edge
374:    fn left_of_uses_self_width
403:    fn below_stacks_vertically
424:    fn above_uses_self_height
453:    fn dangling_reference_falls_back_to_auto
474:    fn cycle_two_outputs_falls_back_to_auto_for_second
493:    fn disabled_output_is_skipped_in_chain
515:    fn explicit_primary_overrides_first
531:    fn multiple_primary_keeps_first_in_placements
550:    fn no_enabled_output_yields_no_primary
563:    fn connected_order_preserved_in_result
583:    fn placement_for_returns_existing_or_none
593:    fn from_config_entry_maps_auto_position
602:    fn from_config_entry_maps_coord_position
616:    fn from_config_entry_maps_each_relation
649:    fn from_config_entry_preserves_primary_and_enabled
659:    fn from_config_entries_builds_layout_in_order
673:    fn empty_layout_single_output_matches_legacy_x_offset_behavior
687:    fn empty_layout_two_outputs_matches_legacy_x_offset_chain
701:    fn empty_layout_three_outputs_matches_legacy_x_offset_chain
717:    fn from_config_entries_empty_yields_empty_layout
723:    fn parse_output_transform_recognizes_all_known_variants
744:    fn parse_output_transform_is_case_insensitive
753:    fn parse_output_transform_unknown_falls_back_to_normal
758:    fn parse_output_transform_trims_whitespace
763:    fn safety_net_force_enables_when_all_disabled
780:    fn safety_net_noop_when_at_least_one_enabled
800:    fn safety_net_noop_when_resolved_empty
807:    fn single_monitor_default_config_remains_enabled_with_normal_transform_scale_one
821:    fn single_monitor_disabled_via_config_safety_net_re_enables
840:    fn diff_no_change_when_both_none
852:    fn diff_no_change_when_identical_modes
863:    fn diff_mode_changed_when_dimensions_differ
874:    fn diff_mode_changed_when_refresh_differs
885:    fn diff_mode_changed_when_one_side_is_none
896:    fn diff_enabled_changed_toggles
907:    fn diff_enabled_unchanged_when_default_true_both_sides
914:    fn diff_combined_mode_and_enabled_change
924:    fn placement
938:    fn connected
946:    fn expected_output
966:    fn config_entry
979:    fn config_entry_with_mode
990:    fn mode
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

<details><summary><code>crates/meridian-compositor/src/state/output_registry.rs</code> &mdash; 611 lines</summary>

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
229:    fn reg
244:    fn reconfigure
260:    fn register_and_list_outputs
269:    fn output_modes_are_stored_by_output_id
288:    fn primary_and_first_fallback_work
304:    fn lookup_by_id_works
315:    fn output_at_point_handles_two_horizontal_outputs
335:    fn select_for_point_with_fallback_prefers_point_match
348:    fn select_for_point_with_fallback_uses_primary_before_first
393:    fn select_for_point_with_fallback_uses_first_when_no_primary_exists
438:    fn select_for_point_with_fallback_is_none_when_empty
446:    fn empty_registry_is_safe
456:    fn remove_by_id_removes_output
467:    fn remove_unknown_output_is_safe_noop
476:    fn reconfigure_keeps_output_id
485:    fn reconfigure_updates_geometry_and_scale
500:    fn primary_fallback_works_after_primary_remove
511:    fn output_id_is_not_reused_after_remove_and_add
520:    fn remove_primary_promotes_first_remaining_to_primary
534:    fn remove_non_primary_does_not_change_primary_flag
546:    fn remove_only_output_leaves_empty_registry_without_panic
557:    fn remove_by_name_promotes_first_remaining_to_primary
570:    fn remove_when_no_primary_was_set_still_promotes_first
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

<details><summary><code>crates/meridian-compositor/src/state/setup.rs</code> &mdash; 1186 lines</summary>

```rust
63:pub
69:pub
128:pub
147:fn xkb_value
164:    fn parses_vconsole_layout_and_options
173:    fn handles_quotes_and_ignores_comments
180:pub
190:impl MeridianState
198:    pub fn reapply_output_layout
466:    fn build_and_register_disabled_output
630:    pub fn mark_all_outputs_dirty
653:    pub fn mark_output_dirty
676:    pub fn mark_output_dirty_by_name
700:    pub fn new
853:    fn post_output_state_change
902:    pub fn handle_output_added_or_updated
960:    fn reclamp_windows_to_live_outputs
981:    pub fn handle_output_removed
1022:    pub fn handle_output_reconfigured
1063:    pub fn register_output_info
1067:    pub fn output_geometry_for_registry
1076:    fn init_wayland_listener
1127:    fn apply_config_overrides_marks_cursor_change_when_cursor_override_differs
1147:    fn apply_config_overrides_marks_wallpaper_change_and_updates_override
1173:    fn apply_config_overrides_with_unknown_theme_keeps_current_theme_and_flags_unchanged
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

<details><summary><code>crates/meridian-config/src/config.rs</code> &mdash; 1993 lines</summary>

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
290:    fn panel_pinned_apps_parse_from_toml
316:    fn panel_section_missing_gives_empty_pinned
324:    fn panel_pinned_unknown_field_returns_error
334:    fn missing_file_uses_defaults
343:    fn valid_toml_parses_general_cursor_and_wallpaper
374:    fn invalid_toml_falls_back_to_defaults
384:    fn cursor_and_wallpaper_modes_parse
410:    fn reload_from_path_with_valid_file_updates_all_sections
442:    fn reload_from_path_with_invalid_file_returns_error_and_preserves_old_config
462:    fn set_cursor_in_toml_replaces_existing_section_and_round_trips
493:    fn set_cursor_in_toml_appends_when_absent
505:    fn set_idle_timeout_replaces_existing_key_and_round_trips
519:    fn set_idle_timeout_inserts_into_existing_general_section
530:    fn set_idle_timeout_appends_general_when_absent
542:    fn set_idle_timeout_none_removes_the_key
555:    fn set_idle_timeout_none_on_missing_key_is_noop
561:    fn reload_from_path_with_missing_file_resets_to_defaults
577:    fn keybinds_section_remains_supported
592:    fn reload_from_path_with_invalid_keybind_keeps_previous_config
614:    fn outputs_section_parses_two_outputs_with_relative_position
641:    fn outputs_position_table_variants_parse
680:    fn outputs_position_coord_inline_table_parses
698:    fn outputs_position_string_auto_parses
716:    fn outputs_position_string_other_returns_error
748:    fn outputs_position_multiple_relations_returns_error
762:    fn outputs_position_xy_mixed_with_relation_returns_error
776:    fn outputs_position_only_x_returns_error
790:    fn outputs_position_empty_table_means_auto
808:    fn outputs_defaults_when_only_name_set
828:    fn outputs_mode_table_parses
867:    fn outputs_transform_passes_through_string_unvalidated
892:    fn outputs_section_missing_keeps_empty_vec
907:    fn outputs_unknown_field_returns_error
921:    fn reload_with_outputs_replaces_previous_set
953:    fn outputs_section_preserved_with_other_sections
1004:    fn set_primary_output_updates_existing_output_sections
1031:    fn set_primary_output_appends_missing_output_section
1046:    fn set_output_mode_updates_existing_output_section
1067:    fn set_output_mode_appends_missing_output_section
1078:    fn set_output_key_replaces_existing_scale_in_section
1092:    fn set_output_key_inserts_transform_when_absent_and_round_trips
1106:    fn set_output_key_appends_section_when_output_absent
1114:    fn remove_output_key_drops_transform_only_for_target
1124:    fn output_by_name<'a>
1132:    fn unique_test_path
1145:    fn write
1153:impl MeridianConfig
1156:    pub fn save_theme
1219:    pub fn save_wallpaper
1257:    pub fn save_cursor
1283:    pub fn save_idle_timeout
1304:    pub fn save_pinned_apps
1342:    pub fn save_primary_output
1371:    pub fn save_output_mode
1408:    pub fn save_output_scale
1424:    pub fn save_output_transform
1440:    pub fn scan_wallpaper_dirs
1478:fn strip_toml_section
1503:fn set_cursor_in_toml
1514:fn set_output_mode_in_toml
1579:fn set_primary_output_in_toml
1642:fn output_section_name
1657:fn is_mode_key_line
1665:fn is_primary_key_line
1673:fn push_output_mode_line
1685:fn push_primary_line
1697:fn write_output_key
1718:fn remove_output_key
1732:fn is_output_key_line
1743:fn set_output_key_in_toml
1804:fn remove_output_key_in_toml
1839:fn collect_images_by_dir
1871:fn wallpaper_entry_display_name
1889:fn wallpaper_dir_display_name
1902:fn has_general_section
1908:fn find_theme_line
1915:fn find_general_key_line
1937:fn set_idle_timeout_in_toml
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

<details><summary><code>crates/meridian-config/src/theme/types/config.rs</code> &mdash; 476 lines</summary>

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
254:pub enum WallpaperMode
262:impl fmt::Display for WallpaperMode
263:    fn fmt
274:pub struct Wallpaper
282:pub struct ThemeConfig
291:impl ThemeConfig
292:    pub fn glass_tint_color
298:    pub fn appearance_is_light
310:    fn test_theme_colors_default_is_dark_palette
327:    fn test_decorations_default_central_glass
346:    fn test_glass_tint_color_defaults_none_and_parses
365:    fn test_theme_config_partial_toml_fills_new_defaults
396:    fn surface_treatment_uses_theme_radius_and_glass_alpha
430:    fn surface_treatment_non_glass_is_opaque_without_blur
447:    fn theme_config_appearance_tracks_background_luminance
456:    fn test_fonts_default_uses_inter
462:    fn ui_family_strips_trailing_size
471:    fn test_cursor_default_uses_installed_theme
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

<details><summary><code>crates/meridian-ipc/src/lib.rs</code> &mdash; 796 lines</summary>

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
372:    fn window_snapshot_entry_contains_workspace_id_and_title
387:    fn window_snapshot_event_roundtrip_supports_multiple_workspaces
421:    fn output_workspace_changed_event_roundtrip_supports_optional_name
435:    fn output_workspace_snapshot_event_roundtrip_supports_two_outputs
495:    fn legacy_workspace_changed_roundtrip_remains_stable
503:    fn window_focus_cleared_event_roundtrip_is_supported
511:    fn desktop_context_menu_event_roundtrip_is_supported
519:    fn output_workspace_snapshot_allows_missing_focused_output
537:    fn output_workspace_snapshot_decodes_legacy_output_without_display_details
555:    fn window_snapshot_entry_missing_minimized_decodes_to_false
562:    fn launch_app_command_roundtrip_uses_argv
580:    fn launch_app_command_accepts_legacy_command_field
594:    fn quit_command_roundtrip_is_supported
602:    fn screenshot_bridge_request_supports_full_output_mode
622:    fn screenshot_bridge_request_rejects_empty_request_id
641:    fn screenshot_bridge_request_accepts_region_with_nonzero_size
660:    fn screenshot_bridge_request_rejects_region_with_zero_dimension
684:    fn screenshot_bridge_request_roundtrip_works
710:    fn screenshot_bridge_response_roundtrip_with_error_works
728:    fn screenshot_bridge_response_roundtrip_with_success_works
748:    fn screenshot_bridge_request_metadata_roundtrip_works
773:    fn capture_window_thumbnail_command_roundtrip_is_supported
785:    fn window_thumbnail_event_roundtrip_is_supported
```

</details>

### `meridian-lock`

<details><summary><code>crates/meridian-lock/src/auth.rs</code> &mdash; 162 lines</summary>

```rust
32:struct ConvData
37:impl Drop for ConvData
38:    fn drop
93:struct PamGuard
98:impl Drop for PamGuard
99:    fn drop
109:pub fn authenticate
```

</details>

<details><summary><code>crates/meridian-lock/src/main.rs</code> &mdash; 1135 lines</summary>

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
448:fn create_shm_buffer
502:fn color
510:fn fill_rect
541:fn fill_circle
564:fn draw_lock_icon
621:struct TextMetrics
627:fn measure_text
637:fn draw_text
652:fn draw_text_centered
659:fn render_frame
776:fn create_lock_surface
798:fn render_surface
848:fn get_username
870:fn main
1002:    fn us_keymap_state
1010:    fn test_state
1041:    fn color_unpacks_rgba_channels
1063:    fn typing_appends_characters
1072:    fn backspace_removes_one_ascii_char
1080:    fn backspace_removes_one_full_utf8_char
1092:    fn backspace_on_empty_password_is_noop
1099:    fn escape_clears_password_and_resets_status
1109:    fn auth_in_progress_blocks_input
1120:    fn return_with_empty_password_does_not_start_auth
1128:    fn typing_resets_failed_status_to_idle
```

</details>

### `meridian-login`

<details><summary><code>crates/meridian-login/src/auth.rs</code> &mdash; 483 lines</summary>

```rust
66:pub enum AuthResult
82:pub enum AuthBackend
87:impl AuthBackend
88:    fn pam_service
101:pub struct AuthDriver
106:impl AuthDriver
109:    pub fn close
117:impl Drop for AuthDriver
118:    fn drop
130:pub fn start_auth_session
153:struct ConvData
158:impl Drop for ConvData
159:    fn drop
254:fn drain_pam_env
290:struct PamGuard
295:impl Drop for PamGuard
296:    fn drop
307:fn run_pam_session
447:    fn empty_username_returns_failed_quickly
468:    fn auth_backend_selects_expected_pam_service
477:    fn drain_pam_env_on_null_handle_is_empty
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

<details><summary><code>crates/meridian-login/src/main.rs</code> &mdash; 2822 lines</summary>

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
294:impl LoginUiState
295:    fn apply
349:    fn start_auth
367:    fn poll_auth
383:    fn tick
391:    fn confirm_power_action
406:    fn clear_expired_power_confirmation
418:    fn pending_power_action
424:    fn reject
434:    fn shake_offset
449:    fn hint
485:    fn smartcard_login_ready
489:    fn auth_username
502:    fn update_security_key_state
537:fn yubikey_present
542:fn yubikey_present_in_sysfs
560:fn yubikey_present_in_hidraw_sysfs
573:fn hid_id_vendor_from_uevent
580:fn hid_name_from_uevent
586:fn is_yubico_vendor_id
594:fn is_yubikey_name
599:fn smartcard_user_from_authfile
607:fn keyboard_layout_label
627:fn light_appearance
631:fn load_login_theme
648:fn login_theme
652:fn main
982:struct AnimFrame
987:fn compute_anim_frame
999:fn anim_frame_is_steady
1003:fn ramp_f32
1015:fn run_animation
1249:fn click_target_at
1291:fn login_button_rect
1301:fn smartcard_pin_rect
1313:fn power_button_rects
1329:fn run_power_action
1343:fn color_with_alpha
1352:fn mix_color
1362:fn alpha_byte
1366:fn theme_color
1370:fn modal_fill_alpha
1377:fn modal_frame_alpha
1384:fn metro_surface
1388:fn metro_background
1396:fn metro_accent
1400:fn metro_text
1404:fn metro_text_dim
1408:fn metro_border
1412:fn metro_error
1416:fn metro_success
1420:fn draw_soft_card_shadow
1452:fn draw_card_stroke
1463:fn draw_login_button
1506:fn draw_power_buttons
1544:fn point_in_rect
1548:fn draw_card
1588:fn draw_login_ui
1808:fn caret_x
1816:fn draw_submit_button
1870:fn draw_brand_mark
1929:fn draw_user_icon
1944:fn draw_lock_icon
1959:fn draw_yubikey_icon
2071:fn draw_input_box
2113:fn draw_caret
2130:fn draw_pointer_cursor
2163:fn card_rect
2171:fn rounded_rect_path
2187:fn bootsplash_handover
2191:fn bootsplash_exit
2195:fn bootsplash_socket_path
2199:fn send_command
2228:enum IpcEvent
2248:fn spawn_login_ipc_server
2289:fn handle_login_ipc_client
2318:    fn smartcard_ready_state
2328:    fn anim_frame_at_t0_matches_settle_state
2335:    fn anim_frame_at_ui_fade_end_is_full
2343:    fn anim_frame_reports_steady_after_intro
2350:    fn card_rect_clamped_dimensions
2357:    fn yubikey_detector_matches_yubico_vendor_id
2372:    fn yubikey_detector_accepts_zero_padded_hid_vendor_id
2379:    fn yubikey_detector_matches_yubikey_product_name
2395:    fn hidraw_detector_matches_yubico_hid_id
2414:    fn hidraw_detector_matches_yubikey_hid_name
2433:    fn yubikey_detector_ignores_other_vendor_ids
2448:    fn smartcard_user_reads_first_mapping_user
2466:    fn power_buttons_are_click_targets
2496:    fn login_button_is_submit_target
2512:    fn empty_placeholder_keeps_caret_at_text_start
2518:    fn smartcard_mode_does_not_focus_username_field
2526:    fn smartcard_mode_focuses_short_pin_field
2542:    fn manual_mode_focuses_username_and_password_fields
2581:    fn power_action_requires_second_matching_click
2592:    fn power_confirmation_switches_and_expires
2611:    fn rounded_rect_path_does_not_panic_on_small_inputs
2616:    fn insert_appends_to_pin_field
2632:    fn backspace_removes_last_char_from_pin_field
2644:    fn cycle_focus_keeps_pin_field
2653:    fn manual_mode_edits_username_and_password
2670:    fn manual_mode_cycles_between_username_and_password
2680:    fn submit_and_cancel_return_their_control_flow
2687:    fn reject_keeps_username_and_resets_password_focus
2702:    fn shake_offset_is_zero_outside_failed_state
2708:    fn shake_offset_nonzero_inside_failed_window
2730:    fn hint_changes_with_phase
2740:    fn hint_warns_when_caps_lock_is_active
2752:    fn hint_mentions_yubikey_when_present
2764:    fn hint_allows_password_login_with_unregistered_yubikey
2776:    fn hint_prompts_touch_while_authenticating_with_yubikey
2783:    fn poll_auth_returns_none_when_no_thread_running
2789:    fn start_auth_with_empty_username_returns_failed_quickly
2797:    fn auth_username_uses_manual_username_without_yubikey
2806:    fn auth_username_uses_smartcard_mapping_when_ready
2815:    fn insert_respects_max_field_len
```

</details>

<details><summary><code>crates/meridian-login/src/session.rs</code> &mdash; 264 lines</summary>

```rust
21:pub enum SessionError
31:impl std::fmt::Display for SessionError
32:    fn fmt
45:impl std::error::Error for SessionError
56:pub fn launch_compositor_for
228:fn ensure_runtime_dir
249:    fn unknown_user_yields_user_not_found
255:    fn runtime_dir_path_format
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

<details><summary><code>crates/meridian-polkit/src/wayland.rs</code> &mdash; 620 lines</summary>

```rust
33:pub struct PamResult
38:pub struct ActiveAuth
51:pub struct AppState
73:struct PopupSurface
90:impl AppState
91:    pub fn new
107:    pub fn on_auth_request
144:    fn reload_theme
161:    pub fn on_cancel_from_polkit
170:    pub fn on_pam_result
195:    fn finish
204:    fn ensure_popup
242:    fn destroy_popup
256:    pub fn draw
356:    fn handle_key
419:impl Dispatch<wl_registry::WlRegistry,
420:    fn event
455:impl Dispatch<wl_seat::WlSeat,
456:    fn event
475:impl Dispatch<wl_keyboard::WlKeyboard,
476:    fn event
544:impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
545:    fn event
592:        impl Dispatch<$iface,
593:            fn event
613:pub fn connect
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

<details><summary><code>crates/meridian-shell/src/app_view.rs</code> &mdash; 1137 lines</summary>

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
360:fn draw_header
413:fn draw_bento_strip
498:fn draw_app_grid
591:fn draw_search_results
679:fn draw_app_row_content
709:fn draw_power_footer
807:fn draw_settings_symbol
839:fn arc_seg
861:fn draw_power_symbol
961:fn draw_scrollbar
1001:fn section_label
1012:fn divider
1025:fn divider_col
1029:fn fill_rect
1035:fn fill_round_rect
1041:fn with_alpha
1045:fn blit_rgba_to_argb
1057:fn to_tiny_skia_color
1066:    fn blit_rgba_to_argb_swaps_red_and_blue
1074:    fn blit_twice_roundtrips
1084:    fn hit_bento_tile_correct_columns
1100:    fn hit_app_row_grid_mode
1115:    fn hit_app_row_search_mode
1122:    fn hit_footer_power_btn_range
1131:    fn hit_header_settings_btn
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

<details><summary><code>crates/meridian-shell/src/context_menu.rs</code> &mdash; 1109 lines</summary>

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
335:fn draw_menu_icon
486:fn draw_submenu_arrow_indicator
528:pub
555:fn draw_overlay_with_background
674:pub
734:fn draw_submenu_overlay
824:fn blit_over
875:    fn state
889:    fn item_list_non_terminal_non_pinned_has_five_items
900:    fn item_list_terminal_pinned_has_four_items
910:    fn item_list_non_terminal_pinned_shows_unpin
918:    fn hit_item_above_menu_is_none
925:    fn hit_item_first_row
934:    fn hit_item_last_row
945:    fn contains_point_outside_returns_false
956:    fn clamp_position_fits_inside_launcher
963:    fn clamp_position_right_edge_clamped
969:    fn clamp_position_bottom_edge_flips_up
976:    fn draw_overlay_does_not_panic
993:    fn draw_overlay_modifies_canvas_at_menu_location
1016:    fn desktop_item_list_has_five_items_with_settings_at_idx_three
1030:    fn submenu_items_has_expected_categories
1043:    fn submenu_hit_item_local_returns_correct_index
1051:    fn glass_menu_palette_uses_theme_colors
1071:    fn submenu_hit_item_local_hits_every_row_and_rejects_gap
1092:    fn total_menu_width_grows_when_submenu_open
1098:    fn desktop_hit_item_uses_desktop_coordinates
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

<details><summary><code>crates/meridian-shell/src/default_apps.rs</code> &mdash; 574 lines</summary>

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
342:pub fn query_default
361:pub fn set_default_for_mimes
382:pub fn apply_sensible_defaults_for_empty
413:fn desktop_app_dirs
435:fn parse_mime_candidate
495:    fn category_metadata_matches_per_variant
519:    fn parse_minimal_desktop_entry_with_mimes
542:    fn parse_hidden_entry_is_skipped
552:    fn index_apps_for_mime_returns_only_matching
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

<details><summary><code>crates/meridian-shell/src/draw/painter.rs</code> &mdash; 667 lines</summary>

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
350:    fn pixel_at
355:    fn min_lit_x
372:    fn radius_is_clamped_to_half_extent
378:    fn radius_zero_behaves_like_rect_fill
398:    fn rounded_corners_clip_outer_pixels
417:    fn tiny_rectangles_are_handled_consistently
472:    fn rounded_fill_uses_premultiplied_alpha_write_semantics
490:    fn rounded_fill_full_coverage_pixels_are_premultiplied
508:    fn rounded_fill_outside_corner_pixels_remain_untouched
525:    fn rounded_fill_edge_pixels_use_partial_blending
550:    fn corner_coverage_reports_expected_partial_and_full_values
558:    fn text_centered_uses_fallback_measurement_when_font_missing
574:    fn text_right_aligned_uses_right_padding_when_font_missing
590:    fn draw_image_blits_centered_pixels
616:    fn draw_image_clips_out_of_bounds_without_panic
643:    fn draw_image_alpha_blending_respects_transparent_and_opaque_pixels
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

<details><summary><code>crates/meridian-shell/src/icons/cache.rs</code> &mdash; 276 lines</summary>

```rust
5:enum CacheEntry
10:pub struct IconCache
15:impl IconCache
17:    pub fn new
21:    pub fn new_for_theme
29:    pub
36:    pub fn warm
59:    pub fn lookup
81:    struct TempDir
85:    impl TempDir
86:        fn new
100:        fn path
105:    impl Drop for TempDir
106:        fn drop
111:    fn write_theme_index
128:    fn write_png
141:    fn write_svg
146:    fn create_loader_env
160:    fn warm_is_idempotent_for_existing_key
187:    fn lookup_without_warm_returns_none
194:    fn missing_icon_is_negative_cached_after_warm
208:    fn lookup_returns_stable_reference_without_clone
224:    fn absolute_path_warm_finds_file
239:    fn absolute_path_warm_missing_file_negative_cache
250:    fn absolute_path_warm_downscale_resizes_to_requested
266:    fn absolute_path_svg_warm_finds_file
```

</details>

<details><summary><code>crates/meridian-shell/src/icons/loader.rs</code> &mdash; 1240 lines</summary>

```rust
21:pub
30:struct RccSource
36:struct ThemeLocation
42:struct IconCandidate
49:struct RccCandidate
55:impl IconLoader
56:    pub
77:    pub
92:    pub
104:    fn load_icon_by_name
115:    pub
126:    fn load_icon_from_theme
163:    fn load_from_rcc
187:    fn load_theme
202:    fn theme_chain
239:    fn load_from_pixmaps
249:    fn decode_icon_path
261:    fn decode_svg_file
269:    fn decode_png_file
274:    fn decode_icon_bytes
290:    fn decode_png_bytes
334:fn icon_aliases
341:fn discover_rcc_sources
387:fn select_rcc_candidates
413:fn pick_best_rcc_candidate
444:fn split_basename_and_extension
455:fn infer_nominal_size
468:impl IconLoader
470:    pub
475:    pub
483:pub
515:fn dedupe_paths
526:fn directory_matches_size
538:fn pick_best_candidate
563:fn choose_nearest
572:fn choose_smallest_size
578:fn choose_largest_size
584:fn distance
588:fn decode_to_rgba8
653:fn rgba_to_bgra
664:pub
728:    struct TempDir
732:    impl TempDir
733:        fn new
747:        fn path
752:    impl Drop for TempDir
753:        fn drop
758:    fn write_theme_index
776:    fn write_png_rgba
789:    fn write_png_indexed
803:    fn write_svg_solid_rect
810:    fn make_theme_root
819:    fn build_chain_rcc
887:    fn write_rcc_file
892:    fn lookup_finds_icon_in_theme_directory
916:    fn lookup_falls_back_to_default_theme_before_hicolor
943:    fn lookup_uses_terminal_alias_for_mini_xterm
965:    fn absolute_path_loader_loads_png
980:    fn svg_in_theme_directory_is_loaded
1001:    fn prefers_png_over_svg_in_same_directory
1017:    fn falls_back_to_svg_when_no_png
1038:    fn absolute_path_loader_loads_svg
1052:    fn size_selection_prefers_closest_larger_before_upscaling_smaller
1085:    fn inheritance_chain_finds_parent_theme_icon
1108:    fn pixmaps_fallback_is_used_when_theme_lookup_misses
1129:    fn downscale_produces_requested_dimensions
1152:    fn indexed_palette_png_is_skipped_without_panic
1172:    fn rcc_archives_are_discovered_in_theme_directory
1190:    fn lookup_falls_back_to_rcc_after_fs_exhausted
1212:    fn lookup_prefers_fs_over_rcc_when_both_present
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

<details><summary><code>crates/meridian-shell/src/icons/rcc.rs</code> &mdash; 625 lines</summary>

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
289:    struct TestNode
295:    enum TestNodeKind
307:    fn build_rcc
372:    fn minimal_root_with_file
390:    fn build_rcc_data_names_tree
457:    fn parse_header_returns_none_for_wrong_magic
468:    fn parse_header_returns_none_for_wrong_version
479:    fn parse_header_accepts_v3_with_expected_offsets
488:    fn tree_walk_finds_root_directory_listing
496:    fn tree_walk_navigates_subdirectories
532:    fn tree_walk_works_when_tree_is_last_section
556:    fn read_file_decompresses_zstd_payload
573:    fn read_file_rejects_zlib_compressed_files
584:    fn lookup_nonexistent_path_returns_none
593:    fn integration_real_breeze_rcc_can_be_opened_and_query_known_icon
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

<details><summary><code>crates/meridian-shell/src/launcher.rs</code> &mdash; 767 lines</summary>

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
564:    struct TempDir
568:    impl TempDir
569:        fn new
583:        fn path
588:    impl Drop for TempDir
589:        fn drop
595:    fn make_executable
604:    fn parses_valid_desktop_entry
625:    fn rejects_hidden_nodisplay_and_non_application_entries
643:    fn desktop_visibility_respects_meridian_environment_keys
663:    fn exec_field_codes_are_removed
671:    fn exec_quotes_are_handled
679:    fn parses_categories_and_normalizes_icon_names
696:    fn try_exec_rejects_missing_binary
709:    fn load_from_dirs_deduplicates_sorts_and_checks_try_exec
755:    fn launcher_state_tracks_open_close_and_apps
```

</details>

<details><summary><code>crates/meridian-shell/src/main.rs</code> &mdash; 549 lines</summary>

```rust
125:pub
151:fn install_panic_logger
186:fn chrono_now
195:fn main
236:fn activate_user_session
261:fn redraw_after_ipc
293:fn insert_ipc_event_source
322:fn insert_status_notifier_source
373:fn insert_updates_refresh_ping
401:fn insert_notification_expiry_timer
432:fn insert_notifications_source
478:fn insert_tick_timer
507:fn insert_network_poll_timer
536:    fn default_pinned_apps_contains_expected_entries
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

<details><summary><code>crates/meridian-shell/src/network/nmcli.rs</code> &mdash; 665 lines</summary>

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
477:    fn parse_state_returns_offline_for_unparsable_general
483:    fn parse_state_returns_disconnected_when_general_is_not_connected
492:    fn parse_state_returns_ethernet_when_wired_connected
507:    fn parse_state_returns_wifi_when_wireless_connected
519:    fn parse_state_ignores_loopback
528:    fn parse_state_strips_quoted_colons_in_connection_names
540:    fn parse_wifi_signal_finds_active_network
546:    fn parse_saved_connections_marks_active_and_skips_loopback
562:    fn parse_saved_connections_handles_quoted_colons_in_name
569:    fn activate_connection_args_uses_id_form
577:    fn parse_wifi_networks_dedups_sorts_and_flags_security
601:    fn connect_wifi_invocation_uses_stdin_for_passwords
625:    fn redact_nmcli_args_hides_password_values
648:    fn integration_real_nmcli_can_be_polled
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

<details><summary><code>crates/meridian-shell/src/panel_view.rs</code> &mdash; 1417 lines</summary>

```rust
82:fn build_launcher_icon
146:fn build_audio_icon
234:fn action_for_id_as_click
253:fn status_notifier_label
293:struct PanelDivider;
295:impl Widget for PanelDivider
296:    fn style
306:    fn paint
321:struct PanelChip
329:impl PanelChip
330:    fn new
347:impl Widget for PanelChip
348:    fn id
352:    fn style
362:    fn paint
459:struct PanelPinnedChip
469:impl Widget for PanelPinnedChip
470:    fn id
474:    fn pinned_app_idx
478:    fn launch_info
482:    fn style
492:    fn paint
621:struct PanelWindowChip
630:impl Widget for PanelWindowChip
631:    fn id
635:    fn focus_window_id
639:    fn style
649:    fn paint
704:fn draw_circle
724:fn windows_for_pinned_app
758:fn tint_pixmap_premul
768:pub
1014:fn collect_click_zones
1058:fn apply_frost_noise
1076:fn blit_rgba_to_argb
1089:pub
1251:    fn panel_chip_style_returns_correct_size
1259:    fn tray_chip_widths_match
1266:    fn panel_pinned_chip_pinned_app_idx_returns_idx
1280:    fn panel_pinned_chip_launch_info_returns_program_and_args
1294:    fn panel_window_chip_focus_window_id_returns_id
1306:    fn status_notifier_label_prefers_title_then_icon_then_service
1337:    fn build_panel_widget_tree_root_has_three_children
1369:    fn draw_panel_ui_modifies_canvas_and_fills_clicks
1407:    fn action_for_id_as_click_screenshot
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

<details><summary><code>crates/meridian-shell/src/settings_view.rs</code> &mdash; 4100 lines</summary>

```rust
40:pub enum SettingsCategory
58:impl SettingsCategory
79:    pub fn label
98:    pub fn chip_id
119:    pub fn search_keywords
769:fn with_alpha
773:fn settings_glass_theme_from_config
784:struct SettingsHeaderBar
789:impl Widget for SettingsHeaderBar
790:    fn style
802:    fn paint
808:    fn children
815:struct SettingsBackButton;
817:impl Widget for SettingsBackButton
818:    fn id
822:    fn style
832:    fn paint
852:struct SettingsSearchField
857:impl Widget for SettingsSearchField
858:    fn style
868:    fn paint
889:struct SidebarPanel
896:impl Widget for SidebarPanel
897:    fn style
916:    fn paint
922:    fn children
928:struct SidebarSectionLabel
934:impl Widget for SidebarSectionLabel
935:    fn style
945:    fn paint
957:struct SettingsSidebarRow
964:impl Widget for SettingsSidebarRow
965:    fn id
969:    fn style
979:    fn paint
1021:struct VerticalDivider
1026:impl Widget for VerticalDivider
1027:    fn style
1037:    fn paint
1044:struct ThemeRow
1052:impl Widget for ThemeRow
1053:    fn id
1057:    fn style
1067:    fn paint
1112:struct CursorThemeRow
1120:impl Widget for CursorThemeRow
1121:    fn id
1127:    fn style
1137:    fn paint
1184:struct WallpaperRow
1193:impl Widget for WallpaperRow
1194:    fn id
1198:    fn style
1208:    fn paint
1277:struct WallpaperBrowseRow
1282:impl Widget for WallpaperBrowseRow
1283:    fn id
1287:    fn style
1297:    fn paint
1340:struct PinnedAppLabel
1347:impl Widget for PinnedAppLabel
1348:    fn id
1352:    fn style
1362:    fn paint
1402:struct SettingsPlaceholder
1407:impl Widget for SettingsPlaceholder
1408:    fn style
1418:    fn paint
1429:struct SoundSummaryCard
1435:impl Widget for SoundSummaryCard
1436:    fn style
1446:    fn paint
1505:struct SoundDeviceRow
1517:impl Widget for SoundDeviceRow
1518:    fn id
1527:    fn style
1537:    fn paint
1615:struct PrinterSummaryCard
1621:impl Widget for PrinterSummaryCard
1622:    fn style
1632:    fn paint
1695:struct SystemInfoRow
1701:impl Widget for SystemInfoRow
1702:    fn style
1712:    fn paint
1752:fn default_apps_pick_id
1764:fn default_apps_set_id
1786:struct DefaultAppCategoryRow
1795:impl Widget for DefaultAppCategoryRow
1796:    fn id
1799:    fn style
1808:    fn paint
1866:struct DefaultAppCandidateRow
1875:impl Widget for DefaultAppCandidateRow
1876:    fn id
1879:    fn style
1888:    fn paint
1918:struct NetworkProfileRow
1927:impl Widget for NetworkProfileRow
1928:    fn id
1936:    fn style
1946:    fn paint
2012:struct WifiRow
2022:impl Widget for WifiRow
2023:    fn id
2031:    fn style
2041:    fn paint
2109:struct BluetoothDeviceRow
2119:impl Widget for BluetoothDeviceRow
2120:    fn id
2128:    fn style
2138:    fn paint
2207:struct PrinterRow
2213:impl Widget for PrinterRow
2214:    fn style
2224:    fn paint
2288:fn fit_text
2297:struct DisplayOutputRow
2315:impl Widget for DisplayOutputRow
2316:    fn style
2326:    fn paint
2469:fn display_badge_text
2478:fn display_mode_label
2487:fn selected_display_mode
2502:struct DisplayModeComboButton
2510:impl Widget for DisplayModeComboButton
2511:    fn id
2519:    fn style
2529:    fn paint
2586:struct DisplayModeOptionRow
2595:impl Widget for DisplayModeOptionRow
2596:    fn id
2603:    fn style
2613:    fn paint
2654:struct DisplayPrimaryButton
2660:impl Widget for DisplayPrimaryButton
2661:    fn id
2669:    fn style
2679:    fn paint
2742:struct DisplayCycleButton
2749:impl Widget for DisplayCycleButton
2750:    fn id
2754:    fn style
2764:    fn paint
2793:struct AddAppRow
2801:impl Widget for AddAppRow
2802:    fn id
2806:    fn style
2816:    fn paint
2853:struct Divider
2858:impl Widget for Divider
2859:    fn style
2869:    fn paint
2877:pub
3989:pub
4082:fn printer_service_message
4090:fn blit_rgba_to_argb
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

<details><summary><code>crates/meridian-shell/src/status_notifier.rs</code> &mdash; 787 lines</summary>

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
312:fn forward_item_activation
365:async fn run
384:async fn forward_activation
403:fn inspect_dbus_menu
471:async fn send_dbus_menu_event
480:async fn fetch_dbus_menu_layout
491:fn parse_dbus_menu_layout
505:fn parse_dbus_menu_item
530:fn property_string
537:fn property_bool
544:fn normalize_menu_label
560:fn normalize_service
564:async fn resolve_item_details
592:async fn read_string_property
608:async fn read_object_path_property
627:fn snapshot_items
642:    fn text_value
646:    fn item_node
651:    fn props
659:    fn normalize_service_trims_input
667:    fn snapshot_items_is_sorted_and_stable
707:    fn normalize_menu_label_removes_mnemonics
714:    fn parse_dbus_menu_layout_filters_hidden_items
744:    fn parse_dbus_menu_layout_keeps_separators_and_children
769:    fn display_entries_flattens_children_with_depth
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

<details><summary><code>crates/meridian-shell/src/theme_export.rs</code> &mdash; 254 lines</summary>

```rust
32:pub
49:fn config_home
59:fn write_config_file
75:pub
114:fn push_color_group
133:pub
154:fn apply_gsettings
163:fn set_gsetting
173:fn triplet
179:fn readable_on
193:    fn light_theme
203:    fn triplet_formats_decimal_rgb
210:    fn readable_on_picks_contrasting_role
221:    fn kdeglobals_dark_has_scheme_groups_and_breeze
235:    fn kdeglobals_light_switches_scheme_name
241:    fn gtk_ini_dark_prefers_dark
249:    fn gtk_ini_light_disables_dark
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

<details><summary><code>crates/meridian-shell/src/wayland/handlers/layer.rs</code> &mdash; 562 lines</summary>

```rust
16:impl LayerShellHandler for MeridianShell
17:    fn closed
132:    fn configure
526:impl MeridianShell
527:    fn panel_output_width_fallback
545:    fn output_height_fallback
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

<details><summary><code>crates/meridian-shell/src/wayland/handlers/pointer.rs</code> &mdash; 1063 lines</summary>

```rust
17:impl PointerHandler for MeridianShell
18:    fn pointer_frame
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

<details><summary><code>crates/meridian-shell/src/wayland/handlers/widget_dispatch.rs</code> &mdash; 809 lines</summary>

```rust
11:impl MeridianShell
12:    pub
81:    fn dispatch_launch_action
101:    fn dispatch_popup_action
128:    fn dispatch_settings_action
479:    fn apply_output_mode_selection
528:    fn dispatch_power_action
559:    fn dispatch_pinned_action
594:    fn persist_pinned_apps_and_redraw
600:    fn add_pinned_app_by_addable_index
633:    pub
687:    pub
704:    pub
770:fn power_action_command
781:fn power_action_command
797:    fn power_off_maps_to_platform_command
806:    fn logout_has_no_external_command
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/init.rs</code> &mdash; 723 lines</summary>

```rust
37:pub
122:pub
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

<details><summary><code>crates/meridian-shell/src/wayland/render.rs</code> &mdash; 1859 lines</summary>

```rust
23:impl MeridianShell
24:    fn signature_hash<T: Hash>
30:    fn theme_render_signature
58:    pub
80:    fn panel_render_signature
124:    fn commit_surface_label
131:    fn commit_reason_label
143:    fn commit_reason_from_repaint
157:    pub
180:    pub
335:    pub
352:    pub
465:    pub
490:    pub
538:    pub
593:    pub
598:    pub
609:    pub
660:    pub
671:    pub
906:    pub
918:    pub
1118:    pub
1128:    pub
1213:    pub
1223:    pub
1300:    pub
1315:    pub
1390:    pub
1474:    pub
1485:    pub
1572:    pub
1586:    pub
1667:    pub
1677:    pub
1758:    pub
1764:    fn write_panel_click_zones_snapshot
1806:fn german_month_name
1827:fn round_buffer_corners
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/screencopy.rs</code> &mdash; 396 lines</summary>

```rust
25:pub
47:impl Drop for ScreenshotCapture
48:    fn drop
68:impl Dispatch<ExtImageCopyCaptureManagerV1,
69:    fn event
80:impl Dispatch<ExtOutputImageCaptureSourceManagerV1,
81:    fn event
92:impl Dispatch<ExtImageCaptureSourceV1,
93:    fn event
104:impl Dispatch<WlShmPool,
105:    fn event
116:impl Dispatch<WlBuffer,
117:    fn event
130:impl Dispatch<ExtImageCopyCaptureSessionV1,
131:    fn event
165:fn issue_frame_capture
240:fn crop_screenshot_region
266:fn is_supported_screenshot_format
270:fn screenshot_buffer_layout
284:impl Dispatch<ExtImageCopyCaptureFrameV1,
285:    fn event
347:pub
371:    fn xrgb_to_rgb_channel_swap
387:    fn screenshot_buffer_layout_rejects_empty_or_oversized_constraints
```

</details>

<details><summary><code>crates/meridian-shell/src/wayland/shell.rs</code> &mdash; 475 lines</summary>

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

<details><summary><code>crates/meridian-shell/src/wayland/state.rs</code> &mdash; 3085 lines</summary>

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
384:impl MeridianShell
385:    pub
393:    pub
401:    fn needs_fast_tick
409:    pub
418:    pub
537:    pub
546:    fn open_desktop_context_menu_from_ipc
583:    pub
589:    pub
605:    pub
623:    pub
641:    pub
651:    pub
666:    pub
690:    pub
704:    pub
722:    pub
741:    fn reassert_region_picker_layer_state
754:    pub
807:    pub
832:    fn close_desktop_context_menu_from_ipc
841:    fn desktop_menu_in_open_debounce
847:    fn apply_ipc_event
1044:    fn handle_config_reloaded
1107:    fn update_focused_title
1118:    fn warm_launcher_icons
1140:    pub
1153:    fn poll_launcher_apps_refresh
1174:    fn toggle_launcher
1240:    pub
1280:    fn open_sound_settings_from_tray
1303:    fn open_network_settings_from_tray
1325:    pub
1370:    pub
1387:    pub
1433:    pub
1451:    pub
1516:    pub
1533:    pub
1591:    pub
1612:    pub
1652:    pub
1687:    pub
1703:    pub
1716:    pub
1770:    pub
1783:    pub
1840:    pub
1865:    pub
1877:    pub
1894:    pub
1899:    pub
1921:    pub
1941:    pub
1954:    pub
1968:    pub
1980:    pub
2011:    pub
2025:    pub
2062:    pub
2203:    pub
2219:    pub
2236:    pub
2257:    fn panel_output_height_fallback
2275:    pub
2298:    fn update_occupied_workspaces
2313:    fn maybe_log_repaint_stats
2346:    fn maybe_log_commit_stats
2378:    fn maybe_log_render_stats
2402:pub
2414:fn hidden_apps_path
2419:fn load_wallpaper_thumbnail
2455:    fn workspace_changed_clamps_workspace_range
2464:    fn panel_global_activation_point_offsets_bottom_panel_y
2476:    fn full_snapshot_recalculates_counts_and_active_workspace
2520:    fn empty_snapshot_marks_all_workspaces_empty
2539:    fn snapshot_with_one_window_marks_single_workspace_occupied
2563:    fn snapshot_workspace_values_out_of_range_are_clamped_safely
2600:    fn window_opened_updates_or_inserts_without_crash
2618:    fn window_closed_is_safe_for_unknown_id
2631:    fn window_closed_removes_existing_window
2644:    fn full_snapshot_preserves_workspace_on_window_entries
2677:    fn stale_focused_window_id_is_cleared_when_no_window_matches
2691:    fn resolve_shell_theme_from_config_applies_cursor_and_wallpaper_overrides
2719:    fn resolve_shell_theme_from_config_fails_for_unknown_theme
2731:    fn panel_theme_signature_changes_when_theme_changes
2742:    fn panel_theme_signature_changes_when_surface_alt_changes
2753:    fn panel_theme_signature_changes_when_border_changes
2764:    fn output_workspace_snapshot_with_two_outputs_is_stored
2803:    fn output_workspace_changed_updates_known_output
2837:    fn output_workspace_changed_unknown_output_is_added_safely
2866:    fn output_workspace_changed_clamps_workspace_and_handles_focus_drop
2899:    fn output_workspace_snapshot_clamps_workspace_values
2927:    fn legacy_workspace_changed_still_works_with_and_without_output_aware_state
2949:    fn panel_active_workspace_prefers_focused_output_id
2977:    fn panel_active_workspace_falls_back_to_focused_flag
2995:    fn panel_active_workspace_falls_back_to_primary_output
3023:    fn panel_active_workspace_falls_back_to_first_output
3051:    fn panel_active_workspace_falls_back_to_legacy_when_unavailable
3069:    fn panel_active_workspace_normalizes_out_of_range
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
