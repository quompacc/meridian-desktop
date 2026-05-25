#!/usr/bin/env python3
"""Generate the Meridian theme family (dark + light) theme.toml files.

Strictly flat (no rounded corners, shadows, glass). Identity comes from a
tight, near-monochrome palette: a deep-navy / chart-paper ground plus a single
steel-blue accent (accent_alt is the same hue, just darker). Semantic colors
are muted so they read as part of the family rather than a rainbow. The colour
moments live in the compass wallpaper, not the chrome.

The matching wallpapers are rendered separately by the real compass renderer:
    cargo run -p meridian-compass-render --example wallpaper --release \\
        -- themes/<name>/assets/wallpaper.png [light]
"""

import os

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
THEMES_DIR = os.path.join(REPO, "themes")

DARK = {
    "name": "meridian",
    "icons": "Papirus-Dark",
    "colors": {
        "background": "#0c1620",
        "surface": "#122231",
        "surface_alt": "#0f1c29",
        "accent": "#5b9bd5",
        "accent_alt": "#4a83bd",
        "text": "#d6e0ec",
        "text_dim": "#8597ab",
        "border": "#243749",
        "error": "#b5685c",
        "warning": "#b89a6a",
        "success": "#6fa08c",
    },
}

LIGHT = {
    "name": "meridian-light",
    "icons": "Papirus",
    "colors": {
        "background": "#ece4d3",
        "surface": "#f4efe3",
        "surface_alt": "#e3d9c4",
        "accent": "#2f6299",
        "accent_alt": "#244f7d",
        "text": "#1e2b38",
        "text_dim": "#5d6b78",
        "border": "#c9bca0",
        "error": "#9a4636",
        "warning": "#8a6a30",
        "success": "#3f7d5e",
    },
}


def theme_toml(pal):
    c = pal["colors"]
    return f"""[colors]
background = "{c['background']}"
surface = "{c['surface']}"
surface_alt = "{c['surface_alt']}"
accent = "{c['accent']}"
accent_alt = "{c['accent_alt']}"
text = "{c['text']}"
text_dim = "{c['text_dim']}"
border = "{c['border']}"
error = "{c['error']}"
warning = "{c['warning']}"
success = "{c['success']}"

[decorations]
border_width = 1
corner_radius = 0
shadow = false
shadow_radius = 16
shadow_radius_top = 8
shadow_alpha = 0.18
shadow_offset_y = 0
gap = 8

[fonts]
ui = "Adwaita Sans 11"
mono = "Adwaita Mono 10"

[icons]
theme = "{pal['icons']}"

[cursor]
theme = "Breeze_Light"
size = 24

[wallpaper]
path = "assets/wallpaper.png"
mode = "fill"
"""


def main():
    print("Generating Meridian theme.toml files:")
    for pal in (DARK, LIGHT):
        tdir = os.path.join(THEMES_DIR, pal["name"])
        os.makedirs(os.path.join(tdir, "assets"), exist_ok=True)
        with open(os.path.join(tdir, "theme.toml"), "w") as f:
            f.write(theme_toml(pal))
        print(f"  {pal['name']}/theme.toml")
    print("done. (wallpapers: render via the compass-render `wallpaper` example)")


if __name__ == "__main__":
    main()
