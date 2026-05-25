//! Render a settled, zoomed-out Meridian compass as a desktop wallpaper.
//!
//! Reuses the exact compass renderer from the bootsplash/login so the desktop
//! background visually continues that identity instead of a flat line drawing.
//! The compass sits at a small `radius_factor` (receded, as if the boot logo
//! has zoomed back) with the needle locked north.
//!
//! Usage: `cargo run -p meridian-compass-render --example wallpaper --release \
//!         -- <out.png> [light]`

use std::env;

use meridian_compass_render::{CompassPainter, Fonts, FrameOpts, Style};
use tiny_skia::{Color, Pixmap};

fn c(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

fn dark_style() -> Style {
    Style {
        radius_factor: 0.19,
        ..Style::default()
    }
}

fn light_style() -> Style {
    // Chart-paper recolor: cream radial ground, navy-ink linework.
    Style {
        radius_factor: 0.19,
        north: c(47, 98, 153, 255),
        south: c(154, 63, 47, 255),
        bg_stops: [
            c(243, 236, 221, 255),
            c(236, 227, 208, 255),
            c(224, 213, 189, 255),
        ],
        meridian: c(47, 98, 153, 36),
        ring: c(60, 72, 86, 170),
        tick_minor: c(90, 104, 120, 120),
        tick_major: c(40, 55, 70, 205),
        rose_main_light: c(250, 246, 238, 235),
        rose_main_dark: c(60, 80, 110, 235),
        rose_filler_light: c(185, 172, 150, 200),
        rose_filler_dark: c(120, 110, 95, 200),
        pivot_outer: c(60, 72, 86, 240),
        pivot_inner: c(40, 55, 75, 255),
        signature: c(60, 72, 86, 150),
        cardinal_other: c(50, 62, 76, 240),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Args: <out.png> [light] [WIDTHxHEIGHT]. The size should match the target
    // display aspect ratio — `Fill` scales the image straight onto the output,
    // so a mismatched aspect stretches the compass into an oval. Default is
    // 16:10 (e.g. 1280x800 displays).
    let mut out = "wallpaper.png".to_string();
    let mut light = false;
    let (mut w, mut h) = (2560u32, 1600u32);
    for (i, arg) in env::args().skip(1).enumerate() {
        if i == 0 {
            out = arg;
        } else if arg == "light" {
            light = true;
        } else if let Some((ws, hs)) = arg.split_once('x') {
            w = ws.parse()?;
            h = hs.parse()?;
        }
    }

    let mut pm = Pixmap::new(w, h).ok_or("failed to allocate pixmap")?;

    let style = if light { light_style() } else { dark_style() };
    let painter = CompassPainter::new(Fonts::quompacc())?.with_style(style);
    let mut canvas = pm.as_mut();
    painter.render(
        &mut canvas,
        w as f32,
        h as f32,
        6.0,
        &FrameOpts {
            force_needle_north: true,
            watermark_alpha: 22,
            ..Default::default()
        },
    );

    std::fs::write(&out, pm.encode_png()?)?;
    println!("wrote {out} ({w}x{h}, light={light})");
    Ok(())
}
