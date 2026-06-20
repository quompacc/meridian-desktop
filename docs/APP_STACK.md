# Meridian — Application Stack (GTK-only)

## The insight (why GTK-only)

**GTK applications draw and theme their own window frame.** The titlebar,
window buttons, **corner rounding and drop shadow** all come from the GTK theme
CSS (`window.csd decoration { border-radius; box-shadow }`, `windowcontrols
button`, `.titlebar`). This means:

- For a **GTK app stack, Meridian does NOT build per-app frames** — it ships a
  generated GTK theme (from `meridian-tokens`) and lets GTK render the frame.
  No reimplementing titlebars / rounding / shadows per app.
- The compositor's **server-side-decoration (SSD) frame** is only needed for
  **non-GTK / XWayland / non-CSD** clients (terminals, some Qt, SDL…).
- **Do not** force SSD (it double-frames CSD clients like Chromium, and GTK
  ignores a forced SSD) and **do not** use `GTK_CSD=0`. Honor the client's
  decoration mode; theme the GTK CSD via the generated theme.

KDE/Qt was dropped because those apps need Plasma session infrastructure
(`ksycoca`/`kded6`) Meridian doesn't provide. See the theming pipeline in
[`crates/meridian-shell/src/theme_export.rs`](../crates/meridian-shell/src/theme_export.rs)
and templates in [`crates/meridian-shell/assets/gtk/`](../crates/meridian-shell/assets/gtk/).

## Hard rules

- **GTK-only** default apps.
- **No AUR** — official repos only (see `CLAUDE.md`). All apps below are in Arch
  `extra`.
- App colour, cursor (Adwaita), icons (Papirus-Dark) and the CSD frame shape all
  derive from the central design tokens via `theme_export`.

## Default app set (modern, all in Arch `extra`)

| Category | App | Pkg | Toolkit | Notes |
|---|---|---|---|---|
| File manager | Nemo | `nemo` | GTK3 | Cinnamon; best-in-class, uses gio/mimeapps |
| Image viewer | Loupe | `loupe` | GTK4/libadwaita | modern, fast |
| Text editor | GNOME Text Editor | `gnome-text-editor` | GTK4 | modern |
| Terminal | Ptyxis | `ptyxis` | GTK4 | newest GNOME terminal (alt: `gnome-console`) |
| Archive manager | File Roller | `file-roller` | GTK3 | standard |
| Documents / PDF | Papers | `papers` | GTK4 | modern Evince successor |
| Video | Celluloid | `celluloid` | GTK + mpv | GTK frontend, keeps the mpv engine |
| Music | Amberol | `amberol` | GTK4/libadwaita | minimal modern player |
| Calculator | GNOME Calculator | `gnome-calculator` | GTK4 | |
| System monitor | GNOME System Monitor | `gnome-system-monitor` | GTK | |
| Disk usage | Baobab | `baobab` | GTK4 | |
| Disks | GNOME Disks | `gnome-disk-utility` | GTK | |
| Web browser | Firefox | `firefox` | GTK | GTK-native; better theme integration than Chromium |
| Email (optional) | Geary | `geary` | GTK | modern GTK mail |

**Replaces** the interim Cinnamon X-Apps set (Xviewer→Loupe, Xed→GNOME Text
Editor, Xreader→Papers) and the KDE defaults (Dolphin/Gwenview/Kate) and Chromium
(→ Firefox). Nemo stays.

Set defaults via `xdg-mime default <app>.desktop <mime>` (writes
`~/.config/mimeapps.list`); GTK apps launch via `gio open` which honors it.
