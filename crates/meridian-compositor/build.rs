use image::{Rgba, RgbaImage};

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn main() {
    let width = 256u32;
    let height = 256u32;
    let mut img = RgbaImage::new(width, height);

    // Catppuccin Mocha radial gradient: dark center bloom on deep navy background.
    // Center: #313244 (surface), edges: #1e1e2e (background).
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let cx = x as f32 - width as f32 / 2.0;
        let cy = y as f32 - height as f32 / 2.0;
        let dist = (cx * cx + cy * cy).sqrt() / (width.min(height) as f32 / 2.0);
        // Smooth falloff: brighter at center, darker at edges
        let t = (1.0_f32 - dist.clamp(0.0, 1.0)).powi(2) * 0.35;
        *pixel = Rgba([
            lerp(0x1e, 0x31, t), // r
            lerp(0x1e, 0x32, t), // g
            lerp(0x2e, 0x44, t), // b
            255,
        ]);
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    img.save(format!("{out_dir}/default_wallpaper.png"))
        .expect("failed to write default wallpaper");

    println!("cargo:rerun-if-changed=build.rs");
}
