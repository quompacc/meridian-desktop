use super::*;
use tiny_skia::Pixmap;

#[test]
fn quompacc_fonts_construct_a_painter() {
    let painter = CompassPainter::new(Fonts::quompacc());
    assert!(painter.is_ok());
}

#[test]
fn garbage_sans_font_yields_build_error() {
    let bad = Fonts {
        sans_bold: b"not a font",
        script: Fonts::quompacc().script,
    };
    assert!(matches!(
        CompassPainter::new(bad),
        Err(BuildError::SansFontInvalid)
    ));
}

#[test]
fn garbage_script_font_yields_build_error() {
    let bad = Fonts {
        sans_bold: Fonts::quompacc().sans_bold,
        script: b"not a font",
    };
    assert!(matches!(
        CompassPainter::new(bad),
        Err(BuildError::ScriptFontInvalid)
    ));
}

#[test]
fn needle_angle_finite_for_relevant_t_range() {
    for t in [0.0_f32, 0.5, 1.5, 1.6, 2.0, 5.0, SETTLE_T, 60.0] {
        let a = needle_angle_deg(t);
        assert!(a.is_finite(), "needle_angle_deg({}) = {} not finite", t, a);
    }
}

#[test]
fn needle_angle_at_settle_is_near_north() {
    let a = needle_angle_deg(SETTLE_T);
    let off = (a - 1080.0).abs();
    assert!(off < 5.0, "settle angle {} too far from 1080°", a);
}

#[test]
fn renders_without_panic_at_typical_resolutions() {
    let painter = CompassPainter::new(Fonts::quompacc()).unwrap();
    for (w, h) in [(1280u32, 720u32), (1920, 1080), (1920, 1440), (2560, 1440)] {
        let mut pm = Pixmap::new(w, h).expect("pixmap");
        painter.render(
            &mut pm.as_mut(),
            w as f32,
            h as f32,
            SETTLE_T,
            &FrameOpts::default(),
        );
        let idx = ((h / 2) as usize * w as usize + (w / 2) as usize) * 4;
        let data = pm.data();
        assert!(
            data[idx] != 0 || data[idx + 1] != 0 || data[idx + 2] != 0,
            "center pixel fully black at {}x{}",
            w,
            h
        );
    }
}

#[test]
fn veil_alpha_255_yields_black_frame() {
    let painter = CompassPainter::new(Fonts::quompacc()).unwrap();
    let mut pm = Pixmap::new(640, 480).unwrap();
    painter.render(
        &mut pm.as_mut(),
        640.0,
        480.0,
        SETTLE_T,
        &FrameOpts {
            veil_alpha: 255,
            ..Default::default()
        },
    );
    for chunk in pm.data().chunks_exact(4) {
        assert_eq!(
            (chunk[0], chunk[1], chunk[2]),
            (0, 0, 0),
            "veil 255 should leave RGB all zero"
        );
    }
}

#[test]
fn north_glow_position_consistent_with_needle_angle() {
    let painter = CompassPainter::new(Fonts::quompacc()).unwrap();
    let (w, h) = (1920.0_f32, 1080.0_f32);
    let t = SETTLE_T;
    let (x, y) = painter.north_glow_position(w, h, t);

    let cx = w / 2.0;
    let cy = h / 2.0;
    let r = (w.min(h) * painter.style.radius_factor).round();
    let length = r * 0.78;
    let angle = needle_angle_deg(t);
    let rad = (angle - 90.0).to_radians();
    let expected_x = cx + length * rad.cos();
    let expected_y = cy + length * rad.sin();

    assert!((x - expected_x).abs() < 1e-3);
    assert!((y - expected_y).abs() < 1e-3);
}

#[test]
fn glow_base_radius_matches_internal_geometry() {
    let painter = CompassPainter::new(Fonts::quompacc()).unwrap();
    let (w, h) = (1920.0_f32, 1080.0_f32);
    let expected = (w.min(h) * painter.style.radius_factor).round() * 0.78;
    assert!((painter.glow_base_radius(w, h) - expected).abs() < 1e-3);
}

#[test]
fn render_glow_at_alone_lights_up_pixels() {
    let painter = CompassPainter::new(Fonts::quompacc()).unwrap();
    let (w, h) = (640u32, 480u32);
    let mut pm = Pixmap::new(w, h).unwrap();
    // start from a known dark frame so the glow is the only thing adding light
    painter.render_glow_at(
        &mut pm.as_mut(),
        w as f32 / 2.0,
        h as f32 / 2.0,
        painter.glow_base_radius(w as f32, h as f32),
    );
    // some pixel must have a nonzero blue channel (cyan glow has high B)
    let any_blue = pm.data().chunks_exact(4).any(|p| p[2] > 0);
    assert!(any_blue, "glow contributed no blue pixels");
}

#[test]
fn measure_text_width_is_positive_for_non_empty() {
    let painter = CompassPainter::new(Fonts::quompacc()).unwrap();
    let w = painter.measure_text_width(TextStyle::SansBold(24.0), "Test");
    assert!(w > 0.0);
    assert_eq!(
        painter.measure_text_width(TextStyle::SansBold(24.0), ""),
        0.0
    );
}

#[test]
fn render_text_left_returns_pen_past_last_glyph() {
    let painter = CompassPainter::new(Fonts::quompacc()).unwrap();
    let mut pm = Pixmap::new(800, 200).unwrap();
    let end = painter.render_text_left(
        &mut pm.as_mut(),
        TextStyle::SansBold(24.0),
        "Hi",
        100.0,
        100.0,
        Color::from_rgba8(255, 255, 255, 255),
    );
    let width = painter.measure_text_width(TextStyle::SansBold(24.0), "Hi");
    assert!((end - (100.0 + width)).abs() < 1e-3);
}

#[test]
fn watermark_alpha_dims_compass_toward_background() {
    let painter = CompassPainter::new(Fonts::quompacc()).unwrap();
    let (w, h) = (1280u32, 720u32);
    let mut plain = Pixmap::new(w, h).unwrap();
    let mut dimmed = Pixmap::new(w, h).unwrap();
    painter.render(
        &mut plain.as_mut(),
        w as f32,
        h as f32,
        SETTLE_T,
        &FrameOpts::default(),
    );
    painter.render(
        &mut dimmed.as_mut(),
        w as f32,
        h as f32,
        SETTLE_T,
        &FrameOpts {
            include_north_glow: true,
            watermark_alpha: 200,
            veil_alpha: 0,
            ..Default::default()
        },
    );
    let differ = plain
        .data()
        .iter()
        .zip(dimmed.data().iter())
        .any(|(a, b)| a != b);
    assert!(differ, "watermark_alpha=200 left the frame unchanged");
}

#[test]
fn north_glow_disabled_changes_some_pixels() {
    // At the needle tip itself, the solid needle paint overpaints the glow
    // so they look identical there — but the glow halo extends past the
    // needle and must therefore differ somewhere in the buffer.
    let painter = CompassPainter::new(Fonts::quompacc()).unwrap();
    let (w, h) = (1920u32, 1080u32);
    let mut with_glow = Pixmap::new(w, h).unwrap();
    let mut without_glow = Pixmap::new(w, h).unwrap();

    painter.render(
        &mut with_glow.as_mut(),
        w as f32,
        h as f32,
        SETTLE_T,
        &FrameOpts::default(),
    );
    painter.render(
        &mut without_glow.as_mut(),
        w as f32,
        h as f32,
        SETTLE_T,
        &FrameOpts {
            include_north_glow: false,
            ..Default::default()
        },
    );

    let differ = with_glow
        .data()
        .iter()
        .zip(without_glow.data().iter())
        .any(|(a, b)| a != b);
    assert!(differ, "frames identical with and without north glow");
}
