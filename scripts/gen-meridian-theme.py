#!/usr/bin/env python3
"""Generate the two Meridian theme.toml files (dark + light).

Per the GUI-centralization model each theme file carries ONLY the colour
table (plus icon set + wallpaper). Geometry, radii, shadows and glass live in
the central Rust defaults (`meridian-config` Decorations::default), so the two
themes are identical in everything but colour. The colours here mirror
`meridian_tokens::Palette::DARK` / `::LIGHT` — keep the two in sync.

The matching wallpapers are rendered separately by the real compass renderer:
    cargo run -p meridian-compass-render --example wallpaper --release \\
        -- themes/<name>/assets/wallpaper.png [light]
"""

import os

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
THEMES_DIR = os.path.join(REPO, "themes")

DARK = {
    "name": "dark",
    "icons": "Papirus-Dark",
    "colors": {
        "background": "#14171b",
        "surface": "#20252b",
        "surface_alt": "#1b1f24",
        "accent": "#4e99f3",
        "accent_alt": "#4383ce",
        "text": "#dcdee1",
        "text_dim": "#888d93",
        "border": "#353a40",
        "error": "#b5685c",
        "warning": "#b89a6a",
        "success": "#6fa08c",
    },
}

LIGHT = {
    "name": "light",
    "icons": "Papirus",
    "colors": {
        "background": "#ece4d3",
        "surface": "#f4efe3",
        "surface_alt": "#e3d9c4",
        "accent": "#2f6299",
        "accent_alt": "#244f7d",
        "text": "#07111c",
        "text_dim": "#263746",
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

[icons]
theme = "{pal['icons']}"

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
