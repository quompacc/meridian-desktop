//! Render a settled, zoomed-out Meridian compass as a desktop wallpaper.
//!
//! Reuses the exact compass renderer from the bootsplash/login so the desktop
//! background visually continues that identity instead of a flat line drawing.
//! The compass sits at a small `radius_factor` (receded, as if the boot logo
//! has zoomed back) with the needle locked north.
//!
//! Usage: `cargo run -p meridian-compass-render --example wallpaper --release \
//!         -- <out.png> [light] [WIDTHxHEIGHT]`

use std::env;

use meridian_compass_render::{CompassPainter, Fonts, FrameOpts, Style};
use tiny_skia::Pixmap;

const WALLPAPER_RADIUS: f32 = 0.19;

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

    let base = if light {
        Style::chart()
    } else {
        Style::default()
    };
    let style = Style {
        radius_factor: WALLPAPER_RADIUS,
        ..base
    };
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
