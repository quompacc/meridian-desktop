# GUI Centralization — Native Rust Contract

> **STATUS: BINDING CENTRALIZATION CONTRACT.** Updated 2026-08-25.

## 1. Goal

Every Meridian-owned UI uses one design source and one reusable native component
model. Changing theme color, interaction state, elevation, radius or configured
decoration geometry must not require editing multiple renderers or applications.

## 2. Authoritative sources

- `meridian-tokens`: `Palette`, `Interaction`, `Elevation`, `Radius`
- `meridian-config`: `Decorations` and user-selected configuration
- `meridian_design_manifest.md`: visual and product rules

Native Rust code consumes these types directly. Preview artifacts and the
archived WebKit prototype are never a second editable source of truth.

## 3. Product pipeline

```text
manifest
   │
   ▼
meridian-tokens + meridian-config
   │
   ▼
shared meridian-ui primitives
   ├─ panel
   ├─ launcher
   ├─ Quick Settings
   ├─ login / lock
   └─ later system tools
```

## 4. Non-negotiable invariants

- exactly two themes, light and dark, identical except for color tables
- no local color, alpha, radius, mix or geometry constants in production UI
- explicit guard rationale for unavoidable brand assets or test fixtures
- no compass/brand theater in everyday UI
- shared primitives own focus, hover, pressed, disabled and accessibility states
- external applications are not forced to adopt Meridian geometry
- no parallel renderer or independently evolving design system

## 5. Native quality sequence

1. stabilize the panel's geometry, content, input and output-scale behavior;
2. bring the launcher to the archived prototype's visual quality and preserve
   its keyboard-first application-catalog behavior;
3. implement one coherent native Quick Settings surface;
4. verify both themes, accessibility, scale and input paths;
5. record start-up, idle and interaction performance on the OpenBSD Acer;
6. only then expand Settings, notifications, overview and system tools.

## 6. Performance and invalidation

- cache decoded icons and visual assets by identity, scale and theme
- do not recompute static shadows or graphics every frame
- update state from events, not polling
- invalidate only affected component state
- record cold-start, first-paint, idle CPU/GPU and resident-memory budgets
- provide reduced-motion/effect fallbacks without changing layout geometry

## 7. Guard strategy

The Rust design and source-size guards remain mandatory. They must reject new
literal production colors, alpha values, local radii, unapproved geometry,
duplicate theme tables and files above the project size limit. Exceptions need
the narrow documented `guard:allow` form required by the repository rules.

## 8. Definition of Done

Centralization is complete only when:

1. the manifest remains the highest visual authority;
2. Rust types and validated config are the only editable token source;
3. panel, launcher and Quick Settings use shared native primitives without local
   production design constants;
4. design and source-size guards are green;
5. both themes have identical layout, radius, blur and shadow geometry;
6. theme and config changes propagate without restarting the compositor;
7. caches have explicit invalidation and idle measurements;
8. branding remains limited to the manifest-approved surfaces;
9. no WebKit, GTK or alternate shell renderer is required for the desktop.

Historical line-by-line findings and the WebKit mockup remain available as
evidence. They do not override this contract.
