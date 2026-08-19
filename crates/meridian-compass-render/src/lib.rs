//! Compass renderer for the QuompaCC / Meridian visual identity.
//!
//! guard:allow-file: Marken-/Brand-Asset (Kompassrose). Laut Design-Manifest §11
//! (Login) und §12 (Bootsplash) ist die Kompass-Illustration dort ausdrücklich
//! erlaubt; sie ist eine in sich geschlossene Grafik mit eigenen hell/dunkel-
//! Paletten (`Style::default()` dunkel, `Style::chart()` hell), die der Aufrufer
//! per Theme-Erscheinung wählt. KEINE Alltags-UI — nur Login/Boot. Deshalb sind
//! die rohen Illustrationsfarben hier zulässig (Manifest §3.4/§9/§14: kein
//! Kompass in der Taskbar/Alltags-UI bleibt durch den Guard erzwungen).
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
    SansRegularFontInvalid,
    ScriptFontInvalid,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SansFontInvalid => write!(f, "sans_bold font failed to parse"),
            Self::SansRegularFontInvalid => write!(f, "sans_regular font failed to parse"),
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
    /// Adwaita Sans Regular — calm UI labels and display text.
    SansRegular(f32),
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
    sans_regular: FontRef<'static>,
    script: FontRef<'a>,
    style: Style,
}

impl<'a> CompassPainter<'a> {
    pub fn new(fonts: Fonts<'a>) -> Result<Self, BuildError> {
        Ok(Self {
            sans_bold: FontRef::try_from_slice(fonts.sans_bold)
                .map_err(|_| BuildError::SansFontInvalid)?,
            sans_regular: FontRef::try_from_slice(assets::ADWAITA_SANS_REGULAR)
                .map_err(|_| BuildError::SansRegularFontInvalid)?,
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
            TextStyle::SansRegular(size) => (&self.sans_regular, size),
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

include!("lib/background_and_scale.rs");
include!("lib/rose_and_needle.rs");
include!("lib/text.rs");

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
