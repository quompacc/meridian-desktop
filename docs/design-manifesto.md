# Meridian Design Manifesto

## Positioning
Meridian is a calm, modern Wayland desktop positioned between GNOME and KDE.

It is built for users who want a polished default experience, clear behavior, and practical power without clutter.

## Core Principle
Meridian prioritizes coherent defaults and protocol-correct behavior over endless tweakability and app-specific exceptions.

## Design Rules
1. Wayland-first by default.
2. Toolkit-neutral behavior and presentation.
3. Strong defaults over configuration sprawl.
4. Productive workflows without unnecessary complexity.
5. Small, focused, behavior-safe patches.
6. Diagnostics are temporary and must be cleaned up after use.

## Product Direction
Meridian is developed as a complete desktop experience, not only a compositor.

The direction is:
- predictable interaction and visual consistency
- stable runtime behavior on real hardware
- practical performance without sacrificing quality
- clear boundaries between product policy and implementation details

## Default Tooling Philosophy
Meridian favors built-in, integrated tools where they improve coherence and reduce user setup burden.

Defaults should be strong enough for everyday use, while still allowing advanced users to layer additional tools where needed.

## Non-Goals
1. Unlimited configuration surfaces that degrade coherence.
2. Toolkit- or app-specific policy hacks as primary strategy.
3. Global behavior toggles that trade correctness for one-off compatibility wins.
4. Broad refactors without a scoped, testable migration path.

## Decision Test
A change fits Meridian when it answers yes to all of the following:
1. Does it preserve protocol correctness?
2. Does it improve or protect desktop coherence?
3. Does it keep defaults strong and complexity bounded?
4. Can it be shipped as a small, testable patch?
5. Does it avoid app-specific behavior policy unless strictly necessary?
