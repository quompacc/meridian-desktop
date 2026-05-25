//! Compass renderer for the QuompaCC / Meridian visual identity.
//!
//! This crate is the single source of truth for the compass mark used by the
//! bootsplash (animated, full duration) and meridian-login (static settle
//! frame today, Phase 4 fall-and-morph animation later). It performs no I/O
//! and no DRM work — callers pass in a [`tiny_skia::PixmapMut`] to draw into.
//!
//! ```no_run
//! use meridian_compass_render::{CompassPainter, Fonts, FrameOpts, SETTLE_T};
//! use tiny_skia::Pixmap;
//!
//! let painter = CompassPainter::new(Fonts::quompacc()).unwrap();
//! let mut pm = Pixmap::new(1920, 1080).unwrap();
//! painter.render(&mut pm.as_mut(), 1920.0, 1080.0, SETTLE_T, &FrameOpts::default());
//! ```

#![forbid(unsafe_code)]

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use tiny_skia::{
    BlendMode, Color, FillRule, Paint, PathBuilder, PixmapMut, Rect, Shader, Stroke, Transform,
};

mod assets;

/// `t` value (seconds) chosen so the bootsplash spin-and-settle animation has
/// fully decayed. Useful as a default for static "settle-state" renders.
pub const SETTLE_T: f32 = 10.0;

/// Font byte slices passed into [`CompassPainter::new`]. Use [`Fonts::quompacc`]
/// for the default QuompaCC type pairing (DejaVu Sans Bold + Italianno Regular).
#[derive(Clone, Copy)]
pub struct Fonts<'a> {
    pub sans_bold: &'a [u8],
    pub script: &'a [u8],
}

impl Fonts<'static> {
    /// The embedded QuompaCC default fonts.
    pub fn quompacc() -> Self {
        Self {
            sans_bold: assets::DEJAVU_SANS_BOLD,
            script: assets::ITALIANNO_REGULAR,
        }
    }
}

/// Errors while constructing a [`CompassPainter`].
#[derive(Debug)]
pub enum BuildError {
    SansFontInvalid,
    ScriptFontInvalid,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SansFontInvalid => write!(f, "sans_bold font failed to parse"),
            Self::ScriptFontInvalid => write!(f, "script font failed to parse"),
        }
    }
}

impl std::error::Error for BuildError {}

/// Which of the embedded QuompaCC fonts to use for a public text-rendering
/// call on [`CompassPainter`]. Includes the desired pixel size.
#[derive(Clone, Copy, Debug)]
pub enum TextStyle {
    /// DejaVu Sans Bold — functional labels (cardinals N/O/S/W, UI labels).
    SansBold(f32),
    /// Italianno Regular — calligraphic accents (the QuompaCC wordmark).
    Script(f32),
}

/// Per-frame rendering knobs.
#[derive(Clone, Debug)]
pub struct FrameOpts {
    /// Whether to draw the cyan glow at the needle tip. Phase 4 turns this off
    /// once the glow has detached and is rendered separately by the caller via
    /// [`CompassPainter::render_glow_at`].
    pub include_north_glow: bool,
    /// Semi-transparent overlay using the background-mid color, drawn after
    /// the compass and before `veil_alpha`. Used to fade the compass to a
    /// watermark intensity while staying in-palette. 0 = compass at full
    /// intensity, 255 = compass fully merged into the background.
    pub watermark_alpha: u8,
    /// Black overlay alpha (0..=255) applied last, for fade-in / fade-out.
    pub veil_alpha: u8,
    /// Force the needle to point exactly north (1080° = 360°×3, the
    /// settled angle), ignoring `t` and any post-spin oscillation. Used
    /// for the bootsplash→login handover frame and the login settle
    /// frame so the needle position matches pixel-perfectly across the
    /// process boundary.
    pub force_needle_north: bool,
}

impl Default for FrameOpts {
    fn default() -> Self {
        Self {
            include_north_glow: true,
            watermark_alpha: 0,
            veil_alpha: 0,
            force_needle_north: false,
        }
    }
}

/// Color and proportion overrides for the compass. The default carries the
/// QuompaCC palette: dark blue gradient, cyan accent, muted-red counterpoint.
#[derive(Clone, Debug)]
pub struct Style {
    /// Compass radius as fraction of `min(width, height)`. Default 0.32.
    pub radius_factor: f32,
    /// Cyan accent — needle north, north cardinal label, heading mark.
    pub north: Color,
    /// Muted red — needle south.
    pub south: Color,
    /// Background radial gradient stops (inner, middle, outer).
    pub bg_stops: [Color; 3],
    /// Radial meridian lines around the compass.
    pub meridian: Color,
    /// Outer + inner scale ring.
    pub ring: Color,
    /// Minor tick marks (every 5°).
    pub tick_minor: Color,
    /// Major tick marks (every 30°).
    pub tick_major: Color,
    pub rose_main_light: Color,
    pub rose_main_dark: Color,
    pub rose_filler_light: Color,
    pub rose_filler_dark: Color,
    pub pivot_outer: Color,
    pub pivot_inner: Color,
    /// QuompaCC wordmark color.
    pub signature: Color,
    /// O/S/W labels (N uses `north`).
    pub cardinal_other: Color,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            radius_factor: 0.32,
            north: Color::from_rgba8(120, 210, 255, 255),
            south: Color::from_rgba8(214, 92, 76, 255),
            bg_stops: [
                Color::from_rgba8(22, 30, 56, 255),
                Color::from_rgba8(10, 14, 28, 255),
                Color::from_rgba8(4, 6, 14, 255),
            ],
            meridian: Color::from_rgba8(70, 100, 160, 36),
            ring: Color::from_rgba8(210, 222, 240, 190),
            tick_minor: Color::from_rgba8(180, 200, 230, 120),
            tick_major: Color::from_rgba8(220, 232, 250, 220),
            rose_main_light: Color::from_rgba8(235, 240, 250, 235),
            rose_main_dark: Color::from_rgba8(95, 110, 140, 235),
            rose_filler_light: Color::from_rgba8(155, 170, 200, 200),
            rose_filler_dark: Color::from_rgba8(70, 80, 100, 200),
            pivot_outer: Color::from_rgba8(40, 50, 70, 240),
            pivot_inner: Color::from_rgba8(220, 230, 245, 255),
            signature: Color::from_rgba8(230, 236, 248, 140),
            cardinal_other: Color::from_rgba8(225, 230, 240, 240),
        }
    }
}

impl Style {
    /// Light "chart paper" palette: cream ground, navy-ink linework. The
    /// luminance counterpart to [`Style::default`] (midnight navy).
    pub fn chart() -> Self {
        Self {
            north: Color::from_rgba8(47, 98, 153, 255),
            south: Color::from_rgba8(154, 63, 47, 255),
            bg_stops: [
                Color::from_rgba8(243, 236, 221, 255),
                Color::from_rgba8(236, 227, 208, 255),
                Color::from_rgba8(224, 213, 189, 255),
            ],
            meridian: Color::from_rgba8(47, 98, 153, 36),
            ring: Color::from_rgba8(60, 72, 86, 170),
            tick_minor: Color::from_rgba8(90, 104, 120, 120),
            tick_major: Color::from_rgba8(40, 55, 70, 205),
            rose_main_light: Color::from_rgba8(250, 246, 238, 235),
            rose_main_dark: Color::from_rgba8(60, 80, 110, 235),
            rose_filler_light: Color::from_rgba8(185, 172, 150, 200),
            rose_filler_dark: Color::from_rgba8(120, 110, 95, 200),
            pivot_outer: Color::from_rgba8(60, 72, 86, 240),
            pivot_inner: Color::from_rgba8(40, 55, 75, 255),
            signature: Color::from_rgba8(60, 72, 86, 150),
            cardinal_other: Color::from_rgba8(50, 62, 76, 240),
            ..Self::default()
        }
    }
}

/// Stateless renderer of the compass mark. Holds parsed fonts and a style;
/// can be reused across frames at any resolution.
pub struct CompassPainter<'a> {
    sans_bold: FontRef<'a>,
    script: FontRef<'a>,
    style: Style,
}

impl<'a> CompassPainter<'a> {
    pub fn new(fonts: Fonts<'a>) -> Result<Self, BuildError> {
        Ok(Self {
            sans_bold: FontRef::try_from_slice(fonts.sans_bold)
                .map_err(|_| BuildError::SansFontInvalid)?,
            script: FontRef::try_from_slice(fonts.script)
                .map_err(|_| BuildError::ScriptFontInvalid)?,
            style: Style::default(),
        })
    }

    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn style(&self) -> &Style {
        &self.style
    }

    /// Mutable access to the style so callers can animate fields (e.g.
    /// `radius_factor`) between frames without rebuilding the painter.
    pub fn style_mut(&mut self) -> &mut Style {
        &mut self.style
    }

    /// Draw one frame of the compass into `pm`. `t` is the animation time in
    /// seconds (relative to the start of the bootsplash spin); use [`SETTLE_T`]
    /// for a static settled frame.
    pub fn render(&self, pm: &mut PixmapMut, w: f32, h: f32, t: f32, opts: &FrameOpts) {
        let cx = w / 2.0;
        let cy = h / 2.0;
        let r = (w.min(h) * self.style.radius_factor).round();
        let needle_length = r * 0.78;
        let visual_t = if opts.force_needle_north { SETTLE_T } else { t };
        let angle = if opts.force_needle_north {
            1080.0
        } else {
            needle_angle_deg(t)
        };

        draw_background(pm, w, h, cx, cy, visual_t, &self.style);
        draw_compass_shadow(pm, cx, cy, r, &self.style);
        draw_meridian_lines(pm, cx, cy, r, &self.style);
        draw_sweep_glint(pm, cx, cy, r, visual_t, &self.style);
        draw_scale_ring(pm, cx, cy, r, &self.style);
        draw_rose_shadow(pm, cx, cy, r * 0.72);
        draw_rose(pm, cx, cy, r * 0.72, &self.style);
        if opts.include_north_glow {
            draw_needle_glow(pm, cx, cy, needle_length, angle, &self.style);
        }
        draw_needle(pm, cx, cy, needle_length, angle, &self.style);
        draw_pivot(pm, cx, cy, &self.style);
        draw_heading_mark(pm, cx, cy, r, &self.style);
        draw_cardinals(pm, &self.sans_bold, cx, cy, r, &self.style);
        draw_signature(pm, &self.script, w, h, &self.style);

        if opts.watermark_alpha > 0 {
            let bg = self.style.bg_stops[1];
            let mut overlay = Paint::default();
            overlay.set_color(Color::from_rgba8(
                (bg.red() * 255.0) as u8,
                (bg.green() * 255.0) as u8,
                (bg.blue() * 255.0) as u8,
                opts.watermark_alpha,
            ));
            let rect = Rect::from_xywh(0.0, 0.0, w, h).unwrap();
            pm.fill_rect(rect, &overlay, Transform::identity(), None);
        }

        if opts.veil_alpha > 0 {
            let mut veil = Paint::default();
            veil.set_color(Color::from_rgba8(0, 0, 0, opts.veil_alpha));
            let rect = Rect::from_xywh(0.0, 0.0, w, h).unwrap();
            pm.fill_rect(rect, &veil, Transform::identity(), None);
        }
    }

    /// Screen-space position of the cyan north glow at animation time `t`.
    /// Phase 4 uses this to pin the falling-bobble animation to a continuous
    /// starting point.
    pub fn north_glow_position(&self, w: f32, h: f32, t: f32) -> (f32, f32) {
        let cx = w / 2.0;
        let cy = h / 2.0;
        let r = (w.min(h) * self.style.radius_factor).round();
        let length = r * 0.78;
        let angle = needle_angle_deg(t);
        let rad = (angle - 90.0).to_radians();
        (cx + length * rad.cos(), cy + length * rad.sin())
    }

    /// Compass-radius for a canvas of (w, h). Useful when sizing auxiliary
    /// elements that should match the compass scale.
    pub fn compass_radius(&self, w: f32, h: f32) -> f32 {
        (w.min(h) * self.style.radius_factor).round()
    }

    /// Default base radius for the north glow (same value used internally
    /// when [`FrameOpts::include_north_glow`] is true). Phase 4 multiplies
    /// this by a scale factor as the glow detaches and grows.
    pub fn glow_base_radius(&self, w: f32, h: f32) -> f32 {
        self.compass_radius(w, h) * 0.78
    }

    /// Renders `text` using one of the embedded QuompaCC fonts. The text is
    /// horizontally centered at `cx` and vertically centered around `cy`.
    pub fn render_text_centered(
        &self,
        pm: &mut PixmapMut,
        style: TextStyle,
        text: &str,
        cx: f32,
        cy: f32,
        color: Color,
    ) {
        let (font, size) = self.text_font_and_size(style);
        draw_text_centered(pm, font, size, cx, cy, text, color);
    }

    /// Renders `text` left-aligned starting at `x` with the given baseline
    /// (in pixels from the top). Returns the pen-x just past the last glyph,
    /// useful for placing carets / appending more text on the same line.
    pub fn render_text_left(
        &self,
        pm: &mut PixmapMut,
        style: TextStyle,
        text: &str,
        x: f32,
        baseline_y: f32,
        color: Color,
    ) -> f32 {
        let (font, size) = self.text_font_and_size(style);
        draw_text_at_baseline(pm, font, size, x, baseline_y, text, color)
    }

    /// Returns the advance width of `text` in the requested font + size, so
    /// callers can lay out lines without committing pixels.
    pub fn measure_text_width(&self, style: TextStyle, text: &str) -> f32 {
        let (font, size) = self.text_font_and_size(style);
        measure_text(font, size, text).total_advance
    }

    fn text_font_and_size(&self, style: TextStyle) -> (&FontRef<'a>, f32) {
        match style {
            TextStyle::SansBold(size) => (&self.sans_bold, size),
            TextStyle::Script(size) => (&self.script, size),
        }
    }

    /// Renders just the cyan north glow at an arbitrary screen position with
    /// a given base radius. Phase 4 animates `(x, y, base_radius)` to make
    /// the glow detach from the needle tip and fall toward screen center.
    pub fn render_glow_at(&self, pm: &mut PixmapMut, x: f32, y: f32, base_radius: f32) {
        let (nr, ng, nb) = (
            (self.style.north.red() * 255.0) as u8,
            (self.style.north.green() * 255.0) as u8,
            (self.style.north.blue() * 255.0) as u8,
        );

        for (radius_mult, alpha) in [(0.18_f32, 24u8), (0.12, 50), (0.08, 110)] {
            let r = base_radius * radius_mult;
            if r < 0.5 {
                continue;
            }
            let circle = PathBuilder::from_circle(x, y, r).unwrap();
            let mut paint = Paint::default();
            paint.set_color(Color::from_rgba8(nr, ng, nb, alpha));
            paint.anti_alias = true;
            paint.blend_mode = BlendMode::Screen;
            pm.fill_path(
                &circle,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

/// Compass needle angle in degrees as a function of animation time.
///
/// 0..1.6s: ease-out cubic spin ending at 1080° (= 0° mod 360, i.e. North).
/// 1.6s..:  damped oscillation around 1080° plus a gentle breathing term.
pub fn needle_angle_deg(t: f32) -> f32 {
    let spin = 1.6_f32;
    let breath = 1.4 * (t * 1.1).sin();
    if t < spin {
        let p = t / spin;
        let eased = 1.0 - (1.0 - p).powi(3);
        eased * 1080.0
    } else {
        let tt = t - spin;
        1080.0 + 70.0 * (-tt * 1.9).exp() * (tt * 6.2).cos() + breath
    }
}

// ---- internal layers ----

fn color_with_alpha(color: Color, alpha: u8) -> Color {
    Color::from_rgba8(
        (color.red() * 255.0) as u8,
        (color.green() * 255.0) as u8,
        (color.blue() * 255.0) as u8,
        alpha,
    )
}

fn draw_background(pm: &mut PixmapMut, w: f32, h: f32, cx: f32, cy: f32, t: f32, style: &Style) {
    let drift_x = (t * 0.18).sin() * w.min(h) * 0.018;
    let drift_y = (t * 0.14).cos() * w.min(h) * 0.014;
    let shader = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(cx + drift_x, cy + drift_y),
        tiny_skia::Point::from_xy(cx, cy),
        (w.max(h)) * 0.75,
        vec![
            tiny_skia::GradientStop::new(0.0, style.bg_stops[0]),
            tiny_skia::GradientStop::new(0.55, style.bg_stops[1]),
            tiny_skia::GradientStop::new(1.0, style.bg_stops[2]),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(style.bg_stops[2]));

    let paint = Paint {
        shader,
        ..Default::default()
    };
    let rect = Rect::from_xywh(0.0, 0.0, w, h).unwrap();
    pm.fill_rect(rect, &paint, Transform::identity(), None);

    let vignette = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(cx, cy),
        tiny_skia::Point::from_xy(cx, cy),
        (w.max(h)) * 0.62,
        vec![
            tiny_skia::GradientStop::new(0.0, Color::from_rgba8(0, 0, 0, 0)),
            tiny_skia::GradientStop::new(0.72, Color::from_rgba8(0, 0, 0, 35)),
            tiny_skia::GradientStop::new(1.0, Color::from_rgba8(0, 0, 0, 150)),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::from_rgba8(0, 0, 0, 80)));
    let vignette_paint = Paint {
        shader: vignette,
        blend_mode: BlendMode::SourceOver,
        ..Default::default()
    };
    pm.fill_rect(rect, &vignette_paint, Transform::identity(), None);
}

fn draw_compass_shadow(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, style: &Style) {
    for (offset_y, radius, alpha) in [(r * 0.10, r * 1.08, 46), (r * 0.05, r * 0.94, 34)] {
        let Some(path) = PathBuilder::from_circle(cx, cy + offset_y, radius) else {
            continue;
        };
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(0, 0, 0, alpha));
        paint.anti_alias = true;
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    let glass = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(cx - r * 0.28, cy - r * 0.34),
        tiny_skia::Point::from_xy(cx, cy),
        r * 1.08,
        vec![
            tiny_skia::GradientStop::new(0.0, color_with_alpha(style.north, 34)),
            tiny_skia::GradientStop::new(0.44, Color::from_rgba8(34, 50, 84, 24)),
            tiny_skia::GradientStop::new(1.0, Color::from_rgba8(0, 0, 0, 0)),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(Color::from_rgba8(0, 0, 0, 0)));
    let Some(glass_path) = PathBuilder::from_circle(cx, cy, r * 1.02) else {
        return;
    };
    let glass_paint = Paint {
        shader: glass,
        blend_mode: BlendMode::Screen,
        anti_alias: true,
        ..Default::default()
    };
    pm.fill_path(
        &glass_path,
        &glass_paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_meridian_lines(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, style: &Style) {
    let mut paint = Paint::default();
    paint.set_color(style.meridian);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: 1.0,
        ..Default::default()
    };

    for i in 0..12 {
        let deg = i as f32 * 30.0;
        let rad = (deg - 90.0).to_radians();
        let (sx, sy) = (cx + r * 0.18 * rad.cos(), cy + r * 0.18 * rad.sin());
        let (ex, ey) = (cx + r * 1.85 * rad.cos(), cy + r * 1.85 * rad.sin());
        let mut pb = PathBuilder::new();
        pb.move_to(sx, sy);
        pb.line_to(ex, ey);
        let path = pb.finish().unwrap();
        pm.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn draw_scale_ring(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, style: &Style) {
    let mut lower_paint = Paint::default();
    lower_paint.set_color(Color::from_rgba8(0, 0, 0, 95));
    lower_paint.anti_alias = true;
    let lower_stroke = Stroke {
        width: 4.2,
        ..Default::default()
    };
    let lower = PathBuilder::from_circle(cx + 1.5, cy + 2.5, r).unwrap();
    pm.stroke_path(
        &lower,
        &lower_paint,
        &lower_stroke,
        Transform::identity(),
        None,
    );

    let mut ring_paint = Paint::default();
    ring_paint.set_color(style.ring);
    ring_paint.anti_alias = true;
    let ring_stroke = Stroke {
        width: 1.6,
        ..Default::default()
    };
    let outer = PathBuilder::from_circle(cx, cy, r).unwrap();
    pm.stroke_path(
        &outer,
        &ring_paint,
        &ring_stroke,
        Transform::identity(),
        None,
    );
    let inner = PathBuilder::from_circle(cx, cy, r * 0.92).unwrap();
    pm.stroke_path(
        &inner,
        &ring_paint,
        &ring_stroke,
        Transform::identity(),
        None,
    );

    let mut highlight_paint = Paint::default();
    highlight_paint.set_color(Color::from_rgba8(255, 255, 255, 75));
    highlight_paint.anti_alias = true;
    highlight_paint.blend_mode = BlendMode::Screen;
    let highlight_stroke = Stroke {
        width: 1.0,
        ..Default::default()
    };
    let highlight = PathBuilder::from_circle(cx - 1.0, cy - 1.5, r * 0.965).unwrap();
    pm.stroke_path(
        &highlight,
        &highlight_paint,
        &highlight_stroke,
        Transform::identity(),
        None,
    );

    let tick_paint_minor = {
        let mut p = Paint::default();
        p.set_color(style.tick_minor);
        p.anti_alias = true;
        p
    };
    let tick_paint_major = {
        let mut p = Paint::default();
        p.set_color(style.tick_major);
        p.anti_alias = true;
        p
    };
    let stroke_minor = Stroke {
        width: 1.0,
        ..Default::default()
    };
    let stroke_major = Stroke {
        width: 2.0,
        ..Default::default()
    };

    for i in 0..72 {
        let deg = i as f32 * 5.0;
        let rad = (deg - 90.0).to_radians();
        let is_major = i % 6 == 0;
        let r_inner = if is_major { r * 0.84 } else { r * 0.89 };
        let mut pb = PathBuilder::new();
        pb.move_to(cx + r_inner * rad.cos(), cy + r_inner * rad.sin());
        pb.line_to(cx + r * 0.92 * rad.cos(), cy + r * 0.92 * rad.sin());
        let path = pb.finish().unwrap();
        if is_major {
            pm.stroke_path(
                &path,
                &tick_paint_major,
                &stroke_major,
                Transform::identity(),
                None,
            );
        } else {
            pm.stroke_path(
                &path,
                &tick_paint_minor,
                &stroke_minor,
                Transform::identity(),
                None,
            );
        }
    }
}

fn draw_sweep_glint(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, t: f32, style: &Style) {
    let deg = (t * 18.0) % 360.0;
    let rad = (deg - 90.0).to_radians();
    let width = 0.18_f32;
    let inner = r * 0.18;
    let outer = r * 0.98;
    let p1 = (
        cx + inner * (rad - width).cos(),
        cy + inner * (rad - width).sin(),
    );
    let p2 = (cx + outer * rad.cos(), cy + outer * rad.sin());
    let p3 = (
        cx + inner * (rad + width).cos(),
        cy + inner * (rad + width).sin(),
    );

    let mut pb = PathBuilder::new();
    pb.move_to(p1.0, p1.1);
    pb.line_to(p2.0, p2.1);
    pb.line_to(p3.0, p3.1);
    pb.close();
    let Some(path) = pb.finish() else {
        return;
    };

    let mut paint = Paint::default();
    paint.set_color(color_with_alpha(style.north, 22));
    paint.anti_alias = true;
    paint.blend_mode = BlendMode::Screen;
    pm.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_rose_shadow(pm: &mut PixmapMut, cx: f32, cy: f32, len_main: f32) {
    let len_filler = len_main * 0.55;
    let offset = len_main * 0.018;
    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(0, 0, 0, 72));
    paint.anti_alias = true;

    for i in 0..8 {
        let deg = i as f32 * 45.0;
        let rad = (deg - 90.0).to_radians();
        let length = if i % 2 == 0 { len_main } else { len_filler };
        let base_half = length * 0.13;
        let ox = offset;
        let oy = offset * 1.45;

        let tip = (cx + ox + length * rad.cos(), cy + oy + length * rad.sin());
        let perp = rad + std::f32::consts::FRAC_PI_2;
        let b1 = (
            cx + ox + base_half * perp.cos(),
            cy + oy + base_half * perp.sin(),
        );
        let b2 = (
            cx + ox - base_half * perp.cos(),
            cy + oy - base_half * perp.sin(),
        );

        let mut pb = PathBuilder::new();
        pb.move_to(cx + ox, cy + oy);
        pb.line_to(b1.0, b1.1);
        pb.line_to(tip.0, tip.1);
        pb.line_to(b2.0, b2.1);
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

fn draw_rose(pm: &mut PixmapMut, cx: f32, cy: f32, len_main: f32, style: &Style) {
    let len_filler = len_main * 0.55;
    for i in 0..8 {
        let deg = i as f32 * 45.0;
        let rad = (deg - 90.0).to_radians();
        let is_main = i % 2 == 0;
        let length = if is_main { len_main } else { len_filler };
        let base_half = length * 0.13;

        let tip = (cx + length * rad.cos(), cy + length * rad.sin());
        let perp = rad + std::f32::consts::FRAC_PI_2;
        let b1 = (cx + base_half * perp.cos(), cy + base_half * perp.sin());
        let b2 = (cx - base_half * perp.cos(), cy - base_half * perp.sin());

        let light = if is_main {
            style.rose_main_light
        } else {
            style.rose_filler_light
        };
        let dark = if is_main {
            style.rose_main_dark
        } else {
            style.rose_filler_dark
        };

        let mut pb1 = PathBuilder::new();
        pb1.move_to(cx, cy);
        pb1.line_to(b1.0, b1.1);
        pb1.line_to(tip.0, tip.1);
        pb1.close();
        let path1 = pb1.finish().unwrap();
        let mut paint1 = Paint::default();
        paint1.set_color(light);
        paint1.anti_alias = true;
        pm.fill_path(
            &path1,
            &paint1,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        let mut pb2 = PathBuilder::new();
        pb2.move_to(cx, cy);
        pb2.line_to(tip.0, tip.1);
        pb2.line_to(b2.0, b2.1);
        pb2.close();
        let path2 = pb2.finish().unwrap();
        let mut paint2 = Paint::default();
        paint2.set_color(dark);
        paint2.anti_alias = true;
        pm.fill_path(
            &path2,
            &paint2,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn draw_needle(pm: &mut PixmapMut, cx: f32, cy: f32, length: f32, compass_deg: f32, style: &Style) {
    let rad = (compass_deg - 90.0).to_radians();
    let perp = rad + std::f32::consts::FRAC_PI_2;
    let base_half = length * 0.045;

    let tip_n = (cx + length * rad.cos(), cy + length * rad.sin());
    let tip_s = (
        cx - length * 0.85 * rad.cos(),
        cy - length * 0.85 * rad.sin(),
    );
    let b1 = (cx + base_half * perp.cos(), cy + base_half * perp.sin());
    let b2 = (cx - base_half * perp.cos(), cy - base_half * perp.sin());

    let shadow_offset = length * 0.012;
    let mut shadow_paint = Paint::default();
    shadow_paint.set_color(Color::from_rgba8(0, 0, 0, 92));
    shadow_paint.anti_alias = true;
    let mut pb_shadow = PathBuilder::new();
    pb_shadow.move_to(b1.0 + shadow_offset, b1.1 + shadow_offset);
    pb_shadow.line_to(tip_n.0 + shadow_offset, tip_n.1 + shadow_offset);
    pb_shadow.line_to(b2.0 + shadow_offset, b2.1 + shadow_offset);
    pb_shadow.line_to(tip_s.0 + shadow_offset, tip_s.1 + shadow_offset);
    pb_shadow.close();
    if let Some(path) = pb_shadow.finish() {
        pm.fill_path(
            &path,
            &shadow_paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    let mut pb_n = PathBuilder::new();
    pb_n.move_to(b1.0, b1.1);
    pb_n.line_to(tip_n.0, tip_n.1);
    pb_n.line_to(b2.0, b2.1);
    pb_n.close();
    let path_n = pb_n.finish().unwrap();
    let mut paint_n = Paint::default();
    paint_n.set_color(style.north);
    paint_n.anti_alias = true;
    pm.fill_path(
        &path_n,
        &paint_n,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let mut pb_s = PathBuilder::new();
    pb_s.move_to(b1.0, b1.1);
    pb_s.line_to(tip_s.0, tip_s.1);
    pb_s.line_to(b2.0, b2.1);
    pb_s.close();
    let path_s = pb_s.finish().unwrap();
    let mut paint_s = Paint::default();
    paint_s.set_color(style.south);
    paint_s.anti_alias = true;
    pm.fill_path(
        &path_s,
        &paint_s,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let mut highlight = Paint::default();
    highlight.set_color(Color::from_rgba8(255, 255, 255, 90));
    highlight.anti_alias = true;
    highlight.blend_mode = BlendMode::Screen;
    let stroke = Stroke {
        width: 1.2,
        ..Default::default()
    };
    let mut pb_hi = PathBuilder::new();
    pb_hi.move_to(b1.0, b1.1);
    pb_hi.line_to(tip_n.0, tip_n.1);
    if let Some(path) = pb_hi.finish() {
        pm.stroke_path(&path, &highlight, &stroke, Transform::identity(), None);
    }
}

fn draw_needle_glow(
    pm: &mut PixmapMut,
    cx: f32,
    cy: f32,
    length: f32,
    compass_deg: f32,
    style: &Style,
) {
    let rad = (compass_deg - 90.0).to_radians();
    let tip = (cx + length * rad.cos(), cy + length * rad.sin());
    let north_rgba8 = (
        (style.north.red() * 255.0) as u8,
        (style.north.green() * 255.0) as u8,
        (style.north.blue() * 255.0) as u8,
    );

    for (radius_mult, alpha) in [(0.18_f32, 24u8), (0.12, 50), (0.08, 110)] {
        let circle = PathBuilder::from_circle(tip.0, tip.1, length * radius_mult).unwrap();
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(
            north_rgba8.0,
            north_rgba8.1,
            north_rgba8.2,
            alpha,
        ));
        paint.anti_alias = true;
        paint.blend_mode = BlendMode::Screen;
        pm.fill_path(
            &circle,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn draw_pivot(pm: &mut PixmapMut, cx: f32, cy: f32, style: &Style) {
    let outer = PathBuilder::from_circle(cx, cy, 8.0).unwrap();
    let pivot_shader = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(cx - 3.0, cy - 4.0),
        tiny_skia::Point::from_xy(cx, cy),
        12.0,
        vec![
            tiny_skia::GradientStop::new(0.0, Color::from_rgba8(255, 255, 255, 210)),
            tiny_skia::GradientStop::new(0.42, style.pivot_inner),
            tiny_skia::GradientStop::new(1.0, style.pivot_outer),
        ],
        tiny_skia::SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(style.pivot_outer));
    let p_outer = Paint {
        shader: pivot_shader,
        anti_alias: true,
        ..Default::default()
    };
    pm.fill_path(
        &outer,
        &p_outer,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    let inner = PathBuilder::from_circle(cx, cy, 4.0).unwrap();
    let mut p_inner = Paint::default();
    p_inner.set_color(style.pivot_inner);
    p_inner.anti_alias = true;
    pm.fill_path(
        &inner,
        &p_inner,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_signature(pm: &mut PixmapMut, font: &FontRef<'_>, w: f32, h: f32, style: &Style) {
    let size = (h * 0.042).clamp(28.0, 56.0);
    let y = h * 0.93;
    draw_text_centered(pm, font, size, w / 2.0, y, "QuompaCC", style.signature);
}

fn draw_cardinals(pm: &mut PixmapMut, font: &FontRef<'_>, cx: f32, cy: f32, r: f32, style: &Style) {
    let size = (r * 0.10).max(14.0);
    let label_radius = r * 0.76;

    let labels = [
        (0.0_f32, "N", style.north),
        (90.0, "O", style.cardinal_other),
        (180.0, "S", style.cardinal_other),
        (270.0, "W", style.cardinal_other),
    ];

    for (deg, text, color) in labels {
        let rad = (deg - 90.0).to_radians();
        let tx = cx + label_radius * rad.cos();
        let ty = cy + label_radius * rad.sin();
        draw_text_centered(pm, font, size, tx, ty, text, color);
    }
}

fn draw_heading_mark(pm: &mut PixmapMut, cx: f32, cy: f32, r: f32, style: &Style) {
    let tip = (cx, cy - r * 0.97);
    let b1 = (cx - 7.0, cy - r * 1.06);
    let b2 = (cx + 7.0, cy - r * 1.06);
    let mut pb = PathBuilder::new();
    pb.move_to(tip.0, tip.1);
    pb.line_to(b1.0, b1.1);
    pb.line_to(b2.0, b2.1);
    pb.close();
    let path = pb.finish().unwrap();
    let mut paint = Paint::default();
    paint.set_color(style.north);
    paint.anti_alias = true;
    pm.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

/// Measurements for a piece of text laid out in a given font + size.
struct TextMetrics {
    total_advance: f32,
    text_h: f32,
    max_y: f32,
}

fn measure_text(font: &FontRef<'_>, size: f32, text: &str) -> TextMetrics {
    let scaled = font.as_scaled(PxScale::from(size));
    let mut total_advance = 0.0_f32;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        total_advance += scaled.h_advance(id);
        if let Some(outline) = scaled.outline_glyph(id.with_scale(PxScale::from(size))) {
            let b = outline.px_bounds();
            min_y = min_y.min(b.min.y);
            max_y = max_y.max(b.max.y);
        }
    }
    let text_h = if max_y.is_finite() {
        max_y - min_y
    } else {
        size
    };
    TextMetrics {
        total_advance,
        text_h,
        max_y,
    }
}

/// Rasterizes `text` starting at left edge `pen_x` and given baseline,
/// returning the pen_x just past the last glyph (useful for caret placement).
fn draw_text_at_baseline(
    pm: &mut PixmapMut,
    font: &FontRef<'_>,
    size: f32,
    mut pen_x: f32,
    baseline_y: f32,
    text: &str,
    color: Color,
) -> f32 {
    let scaled = font.as_scaled(PxScale::from(size));
    let pm_w = pm.width() as i32;
    let pm_h = pm.height() as i32;
    let cr = (color.red() * 255.0) as u8;
    let cg = (color.green() * 255.0) as u8;
    let cb = (color.blue() * 255.0) as u8;
    let opacity = color.alpha();

    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        let advance = scaled.h_advance(id);
        let glyph =
            id.with_scale_and_position(PxScale::from(size), ab_glyph::point(pen_x, baseline_y));
        if let Some(outline) = font.outline_glyph(glyph) {
            let b = outline.px_bounds();
            outline.draw(|gx, gy, alpha| {
                let px = b.min.x as i32 + gx as i32;
                let py = b.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= pm_w || py >= pm_h {
                    return;
                }
                let idx = (py as usize * pm_w as usize + px as usize) * 4;
                let data = pm.data_mut();
                let a = (alpha * opacity * 255.0).clamp(0.0, 255.0) as u32;
                for (i, &c) in [cr, cg, cb].iter().enumerate() {
                    let dst = data[idx + i] as u32;
                    data[idx + i] = ((c as u32 * a + dst * (255 - a)) / 255) as u8;
                }
            });
        }
        pen_x += advance;
    }
    pen_x
}

fn draw_text_centered(
    pm: &mut PixmapMut,
    font: &FontRef<'_>,
    size: f32,
    cx: f32,
    cy: f32,
    text: &str,
    color: Color,
) {
    let m = measure_text(font, size, text);
    let pen_x = cx - m.total_advance / 2.0;
    let baseline_y = cy + m.text_h / 2.0 - m.max_y.max(0.0);
    draw_text_at_baseline(pm, font, size, pen_x, baseline_y, text, color);
}

#[cfg(test)]
mod tests {
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
}
