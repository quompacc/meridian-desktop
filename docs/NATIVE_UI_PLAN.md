# Meridian native UI plan

> **STATUS: BINDING.** Architecture decision of 2026-08-25. Meridian-owned
> production UI remains native Rust. This document overrides older WebKit target
> language in historical audits and prototype notes.

## Decision

The compositor, shell, panel, launcher, Quick Settings, notifications, Settings,
login, lock and boot surfaces use Rust-native rendering. The production
workspace has no GTK or WebKit runtime dependency and no environment switch to
replace native shell surfaces.

The final GTK4/WebKit6 prototype is preserved under
`docs/design-reference/webkit-prototype/`. It is a read-only visual reference,
not executable product code and not a second token source.

## Product architecture

```text
meridian-tokens + meridian-config
               │
               ▼
          meridian-ui
     layout / widgets / effects
               │
               ▼
       native meridian-shell
 panel / launcher / Quick Settings / Settings
               │ typed IPC
               ▼
      compositor and small services
```

- `meridian-tokens` and `meridian-config` remain the only editable design
  source.
- `meridian-ui` owns reusable layout, widgets, state and cached effects.
- `meridian-shell` owns product composition, input and Wayland surface
  lifecycle.
- The compositor owns final stacking, blur/composition, focus and policy.
- Privileged actions stay outside the shell in small platform-specific helpers.

## Native quality round

Work in this order before adding large desktop features:

1. match the accepted prototype proportions and hierarchy in the native panel;
2. bring launcher search, catalogue, Settings navigation and keyboard focus to
   one coherent native component model;
3. rebuild the combined Quick Settings card natively while retaining the proven
   network/audio controls;
4. verify light/dark visual parity, input, accessibility and scale behavior;
5. measure idle CPU/GPU, repaint counts, interaction latency and retained cache
   memory on the OpenBSD Intel reference machine.

## Performance model

- Static surfaces are event-driven and do not animate or repaint while idle.
- Layout is recalculated only after geometry/content invalidation.
- Fonts, decoded icons, shadows and other visual assets are cached by the
  inputs that affect them.
- Blur stays compositor-owned; the shell does not duplicate scene sampling.
- New effects require a cache/invalidation description and hardware evidence.

## Acceptance gates

- Panel, launcher and Quick Settings work without optional runtime flags.
- Super, pointer controls and keyboard focus work across repeated cycles.
- Both themes differ only by their central colour tables.
- The design guard and 600-line source guard remain green.
- OpenBSD `cargo check --workspace` and Meridian workspace tests pass.
- A cold native session has no GTK/WebKit UI processes and remains responsive on
  the Acer reference hardware.
