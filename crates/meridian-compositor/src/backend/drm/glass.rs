//! Liquid-glass titlebar: samples the (separably) blurred scene behind the
//! titlebar and composites a tinted frosted pane over it.
//!
//! The compositor renders the scene *without decorations* into an offscreen
//! texture once per frame, runs a two-pass separable Gaussian blur over it (see
//! `render.rs`), wraps the result in a [`TextureBuffer`], and builds one
//! [`GlassTitlebarElement`] per server-side-decorated window. Each element is a
//! [`TextureRenderElement`] whose `src` is the titlebar's screen rectangle
//! inside that texture; a custom fragment shader installed via
//! `override_default_tex_program` mixes the sampled (blurred) background with
//! the theme tint and rounds the top corners.
//!
//! Technique for attaching the custom shader (override the renderer's texture
//! program for the duration of the inner element's draw) is the same one
//! `clipped_surface` uses; only the shader differs.

use smithay::backend::renderer::gles::{UniformName, UniformType};
use smithay::utils::user_data::UserDataMap;
use smithay::{
    backend::renderer::{
        element::{
            texture::{TextureBuffer, TextureRenderElement},
            Element, Id, Kind, RenderElement,
        },
        gles::{GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture, Uniform},
        utils::{CommitCounter, DamageSet, OpaqueRegions},
    },
    utils::{Buffer, Logical, Physical, Point, Rectangle, Scale, Size, Transform},
};

const GLASS_SHADER_SRC: &str = include_str!("glass.frag");
const BLUR_SHADER_SRC: &str = include_str!("blur.frag");

struct GlassShader(GlesTexProgram);
struct BlurShader(GlesTexProgram);

fn glass_uniform_names() -> [UniformName<'static>; 5] {
    [
        UniformName::new("geo_size", UniformType::_2f),
        UniformName::new("corner_radius", UniformType::_4f),
        UniformName::new("u_tint", UniformType::_3f),
        UniformName::new("u_tint_amount", UniformType::_1f),
        // src region of the titlebar inside the scene texture, normalised UV.
        UniformName::new("u_src", UniformType::_4f),
    ]
}

fn blur_uniform_names() -> [UniformName<'static>; 1] {
    [UniformName::new("u_step", UniformType::_2f)]
}

/// Compile + cache the glass texture shader in the renderer's EGL user data.
pub fn glass_shader(renderer: &mut GlesRenderer) -> Option<GlesTexProgram> {
    if let Some(s) = renderer.egl_context().user_data().get::<GlassShader>() {
        return Some(s.0.clone());
    }
    match renderer.compile_custom_texture_shader(GLASS_SHADER_SRC, &glass_uniform_names()) {
        Ok(prog) => {
            let p = prog.clone();
            renderer
                .egl_context()
                .user_data()
                .insert_if_missing(|| GlassShader(p));
            Some(prog)
        }
        Err(err) => {
            tracing::warn!("glass shader compile failed: {:?}", err);
            None
        }
    }
}

/// Compile + cache the separable Gaussian blur shader.
pub fn blur_shader(renderer: &mut GlesRenderer) -> Option<GlesTexProgram> {
    if let Some(s) = renderer.egl_context().user_data().get::<BlurShader>() {
        return Some(s.0.clone());
    }
    match renderer.compile_custom_texture_shader(BLUR_SHADER_SRC, &blur_uniform_names()) {
        Ok(prog) => {
            let p = prog.clone();
            renderer
                .egl_context()
                .user_data()
                .insert_if_missing(|| BlurShader(p));
            Some(prog)
        }
        Err(err) => {
            tracing::warn!("blur shader compile failed: {:?}", err);
            None
        }
    }
}

/// Parameters for one glass titlebar, emitted by the decoration manager as a
/// marker and turned into a [`GlassTitlebarElement`] once the blur texture for
/// the frame exists.
#[derive(Debug, Clone, Copy)]
pub struct GlassTitlebarInfo {
    /// Titlebar rectangle in logical (output) coordinates.
    pub rect: Rectangle<i32, Logical>,
    /// Per-corner radius (top-left, top-right, bottom-right, bottom-left), px.
    pub radius: [f32; 4],
    /// Tint colour (theme surface), linear-ish 0..1.
    pub tint: [f32; 3],
    /// How strongly the frosted background is pulled toward the tint (0..1).
    pub tint_amount: f32,
    /// Blur radius in physical pixels (used by the separable pre-pass).
    pub blur: f32,
    /// Opacity of the whole frosted pane (0..1). This is the theme's
    /// `glass_alpha` / surface `fill_alpha` made into the single opacity knob:
    /// the shader multiplies its output coverage by this, so a value of 1.0 is
    /// a fully opaque frosted surface and lower values let the wallpaper/scene
    /// behind show through. See GUI_CENTRALIZATION_PLAN §6 phase 1.
    pub fill_alpha: f32,
}

/// A glass titlebar render element: a slice of the blurred-scene texture drawn
/// at the titlebar position, with the glass shader applied.
#[derive(Debug)]
pub struct GlassTitlebarElement {
    inner: TextureRenderElement<GlesTexture>,
    program: GlesTexProgram,
    uniforms: Vec<Uniform<'static>>,
}

impl GlassTitlebarElement {
    /// `texture` is the blurred-scene [`TextureBuffer`] (full output size).
    /// `info.rect` selects the titlebar slice of it and where to draw it.
    pub fn new(
        program: GlesTexProgram,
        texture: &TextureBuffer<GlesTexture>,
        info: GlassTitlebarInfo,
        out_size: (i32, i32),
        texture_scale: (f64, f64),
        scale: Scale<f64>,
    ) -> Self {
        let loc_phys: Point<f64, Physical> = info.rect.loc.to_f64().to_physical(scale);
        let size_log: Size<i32, Logical> = info.rect.size;
        let src: Rectangle<f64, Logical> = Rectangle::new(
            (
                info.rect.loc.x as f64 * texture_scale.0,
                info.rect.loc.y as f64 * texture_scale.1,
            )
                .into(),
            (
                info.rect.size.w as f64 * texture_scale.0,
                info.rect.size.h as f64 * texture_scale.1,
            )
                .into(),
        );

        // The pane opacity (theme glass_alpha / surface fill_alpha) rides on the
        // renderer's standard `alpha` uniform, which the glass shader multiplies
        // into its coverage. This makes glass_alpha the one opacity knob for the
        // whole frosted surface instead of the shell painting its own fill.
        let inner = TextureRenderElement::from_texture_buffer(
            loc_phys,
            texture,
            Some(info.fill_alpha.clamp(0.0, 1.0)),
            Some(src),
            Some(size_log),
            Kind::Unspecified,
        );

        let (ow, oh) = (out_size.0.max(1) as f32, out_size.1.max(1) as f32);
        let u_src = [
            info.rect.loc.x as f32 / ow,
            info.rect.loc.y as f32 / oh,
            info.rect.size.w as f32 / ow,
            info.rect.size.h as f32 / oh,
        ];

        let geo_size = (info.rect.size.w as f32, info.rect.size.h as f32);
        let uniforms = vec![
            Uniform::new("geo_size", geo_size),
            Uniform::new("corner_radius", info.radius),
            Uniform::new("u_tint", info.tint),
            Uniform::new("u_tint_amount", info.tint_amount),
            Uniform::new("u_src", u_src),
        ];

        Self {
            inner,
            program,
            uniforms,
        }
    }
}

impl Element for GlassTitlebarElement {
    fn id(&self) -> &Id {
        self.inner.id()
    }
    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
    }
    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }
    fn src(&self) -> Rectangle<f64, Buffer> {
        self.inner.src()
    }
    fn transform(&self) -> Transform {
        self.inner.transform()
    }
    fn damage_since(
        &self,
        scale: Scale<f64>,
        _commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        // The blurred background changes whenever anything behind moves, so the
        // safe behaviour is to always report full damage over our geometry.
        DamageSet::from_slice(&[Rectangle::from_size(self.geometry(scale).size)])
    }
    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        OpaqueRegions::default()
    }
    fn alpha(&self) -> f32 {
        // Carries the pane opacity (fill_alpha) baked into the inner element.
        self.inner.alpha()
    }
    fn kind(&self) -> Kind {
        Kind::Unspecified
    }
}

impl RenderElement<GlesRenderer> for GlassTitlebarElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        frame.override_default_tex_program(self.program.clone(), self.uniforms.clone());
        let res = RenderElement::<GlesRenderer>::draw(
            &self.inner,
            frame,
            src,
            dst,
            damage,
            opaque_regions,
            cache,
        );
        frame.clear_tex_program_override();
        res
    }

    fn underlying_storage(
        &self,
        _renderer: &mut GlesRenderer,
    ) -> Option<smithay::backend::renderer::element::UnderlyingStorage<'_>> {
        None
    }
}

/// A glass titlebar slot in the render list. The decoration pass emits
/// [`GlassElement::Pending`] as a z-order placeholder; once the frame's blur
/// texture exists, `render.rs` swaps it for [`GlassElement::Ready`]. `Pending`
/// is never drawn (it is replaced before `render_frame` and filtered out of the
/// blur source), so its element methods only need to be valid, not pretty.
#[derive(Debug)]
pub enum GlassElement {
    Pending {
        info: GlassTitlebarInfo,
        id: Id,
        geometry: Rectangle<i32, Physical>,
    },
    Ready(GlassTitlebarElement),
}

impl GlassElement {
    pub fn pending(info: GlassTitlebarInfo, scale: Scale<f64>) -> Self {
        let geometry = info.rect.to_physical_precise_round(scale);
        GlassElement::Pending {
            info,
            id: Id::new(),
            geometry,
        }
    }

    /// The titlebar parameters, if this is still a pending placeholder.
    pub fn pending_info(&self) -> Option<GlassTitlebarInfo> {
        match self {
            GlassElement::Pending { info, .. } => Some(*info),
            GlassElement::Ready(_) => None,
        }
    }
}

impl Element for GlassElement {
    fn id(&self) -> &Id {
        match self {
            GlassElement::Pending { id, .. } => id,
            GlassElement::Ready(e) => e.id(),
        }
    }
    fn current_commit(&self) -> CommitCounter {
        match self {
            GlassElement::Pending { .. } => CommitCounter::default(),
            GlassElement::Ready(e) => e.current_commit(),
        }
    }
    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        match self {
            GlassElement::Pending { geometry, .. } => *geometry,
            GlassElement::Ready(e) => e.geometry(scale),
        }
    }
    fn src(&self) -> Rectangle<f64, Buffer> {
        match self {
            GlassElement::Pending { .. } => Rectangle::default(),
            GlassElement::Ready(e) => e.src(),
        }
    }
    fn transform(&self) -> Transform {
        match self {
            GlassElement::Pending { .. } => Transform::Normal,
            GlassElement::Ready(e) => e.transform(),
        }
    }
    fn damage_since(
        &self,
        scale: Scale<f64>,
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        match self {
            GlassElement::Pending { .. } => DamageSet::default(),
            GlassElement::Ready(e) => e.damage_since(scale, commit),
        }
    }
    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        match self {
            GlassElement::Pending { .. } => OpaqueRegions::default(),
            GlassElement::Ready(e) => e.opaque_regions(scale),
        }
    }
    fn alpha(&self) -> f32 {
        match self {
            GlassElement::Pending { .. } => 1.0,
            GlassElement::Ready(e) => e.alpha(),
        }
    }
    fn kind(&self) -> Kind {
        Kind::Unspecified
    }
}

impl RenderElement<GlesRenderer> for GlassElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        match self {
            GlassElement::Pending { .. } => Ok(()),
            GlassElement::Ready(e) => e.draw(frame, src, dst, damage, opaque_regions, cache),
        }
    }

    fn underlying_storage(
        &self,
        renderer: &mut GlesRenderer,
    ) -> Option<smithay::backend::renderer::element::UnderlyingStorage<'_>> {
        match self {
            GlassElement::Pending { .. } => None,
            GlassElement::Ready(e) => e.underlying_storage(renderer),
        }
    }
}
