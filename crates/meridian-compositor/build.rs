use image::{Rgba, RgbaImage};

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn main() {
    let width = 256u32;
    let height = 256u32;
    let mut img = RgbaImage::new(width, height);

    // Tokyo Night radial gradient: subtle center fade with brighter rim.
    // Center: #24283b (surface), edges: #414868 (border).
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        let cx = x as f32 - width as f32 / 2.0;
        let cy = y as f32 - height as f32 / 2.0;
        let dist = (cx * cx + cy * cy).sqrt() / (width.min(height) as f32 / 2.0);
        let t = dist.clamp(0.0, 1.0);
        let t = t * t * (3.0 - 2.0 * t);
        *pixel = Rgba([
            lerp(0x24, 0x41, t), // r
            lerp(0x28, 0x48, t), // g
            lerp(0x3b, 0x68, t), // b
            255,
        ]);
    }

    let out_dir = std::env::var("OUT_DIR").unwrap();
    img.save(format!("{out_dir}/default_wallpaper.png"))
        .expect("failed to write default wallpaper");

    println!("cargo:rerun-if-changed=build.rs");
}
