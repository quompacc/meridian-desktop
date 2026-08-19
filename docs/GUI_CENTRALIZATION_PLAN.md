# GUI Centralization — Native and Web Migration Plan

> **STATUS: BINDING CENTRALIZATION CONTRACT.** Updated 2026-08-19. The earlier
> native-shell audit is historical context; this document now defines how the
> single design source survives the WebKit migration.

## 1. Goal

Every Meridian-owned UI uses one design source and one reusable component model.
Changing theme color, interaction state, elevation, radius or configured
decoration geometry must not require editing multiple renderers or applications.

## 2. Authoritative sources

- `meridian-tokens`: `Palette`, `Interaction`, `Elevation`, `Radius`
- `meridian-config`: `Decorations` and user-selected configuration
- `meridian_design_manifest.md`: visual/product rules

Native Rust code consumes these types directly. Web UI consumes generated,
versioned CSS custom properties. Hand-maintained CSS values are not a second
source of truth.

## 3. Current state

The native shell, login, lock and compositor already use centralized Rust
tokens to varying degrees, with `design_guard` preventing new render hardcodes.
The current shell is implemented in tiny-skia/native Rust and remains the
fallback during migration.

The WebKit runtime, CSS exporter and Web Component library do not yet exist.

## 4. Target pipeline

```text
manifest
   │
   ▼
meridian-tokens + meridian-config
   ├─ native Rust theme consumers
   ├─ generated CSS variables (light/dark)
   └─ generated metadata for previews/tests
             │
             ▼
shared Meridian Web Components
             │
             ├─ panel
             ├─ launcher
             ├─ Quick Settings
             └─ later system tools
```

## 5. Non-negotiable invariants

- exactly two themes, light and dark, identical except for color tables
- no local color, alpha, radius, mix or geometry constants in production UI
- explicit guard rationale for unavoidable brand assets or test fixtures
- no compass/brand theater in everyday UI
- shared components own focus, hover, pressed, disabled and accessibility states
- external applications are not forced to adopt Meridian geometry
- native and web implementations may coexist only during migration, not as
  independently evolving design systems

## 6. Migration phases

### Phase A — Export contract

- define stable token names and units for CSS
- generate both color tables plus shared geometry/elevation/interaction values
- snapshot-test the output
- extend `design_guard` to CSS/TypeScript/HTML assets

### Phase B — Component foundation

- implement only controls needed by panel, launcher and Quick Settings
- add interaction, keyboard, focus and accessibility tests
- provide a component gallery/diagnostic page loaded from packaged assets

### Phase C — Vertical slice

- panel first, then launcher, then Quick Settings
- compare native/web behavior and both themes
- preserve Wayland roles, render order and IPC semantics
- measure caches, idle work, input latency and memory on the Acer

### Phase D — Broader migration

Only after the vertical slice passes: Settings, notifications, overview and
Meridian system tools. Login/bootsplash require their own security decision.

## 7. Performance and invalidation

- cache decoded icons/assets by identity, scale and theme
- do not recompute static shadows/graphics every frame
- update documents from events, not polling
- invalidate only affected component state
- record cold-start, first-paint, idle CPU/GPU and resident-memory budgets
- provide reduced-motion/effect fallbacks without changing layout geometry

## 8. Guard strategy

The existing Rust guard remains mandatory. Before production web migration it
must additionally detect, with documented fixture/brand exceptions:

- literal production CSS colors and alpha values
- local radii and unapproved geometry
- independent `color-mix()` percentages
- duplicate theme tables
- components bypassing the generated token import

Generated artifacts are verified against source tokens; they are not manually
edited.

## 9. Definition of Done

Centralization is complete only when:

1. the manifest is still the highest visual authority;
2. Rust types/config are the only editable token source;
3. light/dark CSS is generated deterministically from that source;
4. native and web guard tests are green;
5. panel, launcher and Quick Settings use shared components without local
   production design constants;
6. both themes have identical layout, radius, blur and shadow geometry;
7. theme/config changes propagate without restarting the compositor;
8. caches have explicit invalidation and idle measurements;
9. branding remains limited to the manifest-approved surfaces;
10. the native fallback can be removed without losing a unique token or widget
    behavior.

Historical line-by-line findings remain available in the dated audit documents
and Git history. They do not override this contract.
