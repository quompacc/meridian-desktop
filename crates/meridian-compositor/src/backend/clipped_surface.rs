//! Rounded-corner clipping for window content surfaces.
//!
//! Server-side decorated windows are composed of separate pieces: the titlebar
//! and border are drawn as rounded pixel-shader elements (see
//! `decoration::render::elements`), but the client's own content is a live
//! Wayland surface texture. To round its bottom corners we wrap each
//! `WaylandSurfaceRenderElement` in [`ClippedSurfaceRenderElement`], which
//! installs a custom texture shader for the duration of that element's draw:
//! the shader samples the surface and multiplies its alpha by a rounded-box
//! coverage so the corners become transparent and reveal the wallpaper.
//!
//! Technique and shader ported from cosmic-comp's `clipped_surface` (which in
//! turn took it from niri). The `input_to_geo` matrix maps each (sub)surface's
//! texture coordinates into whole-window geometry space, so rounding is
//! relative to the window, not the individual surface.

use cgmath::{Matrix3, Vector2};
use smithay::{
    backend::renderer::{
        element::{
            surface::WaylandSurfaceRenderElement, Element, Id, Kind, RenderElement,
            UnderlyingStorage,
        },
        gles::{GlesError, GlesFrame, GlesRenderer, GlesTexProgram, Uniform, UniformValue},
        utils::{CommitCounter, DamageSet, OpaqueRegions},
    },
    utils::{Buffer, Logical, Physical, Point, Rectangle, Scale, Size, Transform},
};
use smithay::backend::renderer::gles::{UniformName, UniformType};
use smithay::utils::user_data::UserDataMap;

const CLIP_SHADER_SRC: &str = include_str!("clipped_surface.frag");

struct ClippingShader(GlesTexProgram);

fn clip_uniform_names() -> [UniformName<'static>; 3] {
    [
        UniformName::new("geo_size", UniformType::_2f),
        UniformName::new("corner_radius", UniformType::_4f),
        UniformName::new("input_to_geo", UniformType::Matrix3x3),
    ]
}

/// Fetch the clipping texture shader, compiling and caching it in the
/// renderer's EGL context user data on first use.
pub fn clip_shader(renderer: &mut GlesRenderer) -> Option<GlesTexProgram> {
    if let Some(s) = renderer.egl_context().user_data().get::<ClippingShader>() {
        return Some(s.0.clone());
    }
    match renderer.compile_custom_texture_shader(CLIP_SHADER_SRC, &clip_uniform_names()) {
        Ok(prog) => {
            let p = prog.clone();
            renderer
                .egl_context()
                .user_data()
                .insert_if_missing(|| ClippingShader(p));
            Some(prog)
        }
        Err(err) => {
            tracing::warn!("clipped-surface shader compile failed: {:?}", err);
            None
        }
    }
}

/// `radius` order is `[bottom_right, top_right, bottom_left, top_left]`
/// (matches cosmic-comp / the `clipped_surface.frag` corner layout).
#[derive(Debug)]
pub struct ClippedSurfaceRenderElement {
    inner: WaylandSurfaceRenderElement<GlesRenderer>,
    program: GlesTexProgram,
    radius: [u8; 4],
    geometry: Rectangle<f64, Logical>,
    uniforms: Vec<Uniform<'static>>,
}

impl ClippedSurfaceRenderElement {
    pub fn new(
        program: GlesTexProgram,
        elem: WaylandSurfaceRenderElement<GlesRenderer>,
        scale: Scale<f64>,
        geometry: Rectangle<f64, Logical>,
        radius: [u8; 4],
    ) -> Self {
        let elem_geo = elem.geometry(scale);
        let geo: Rectangle<i32, Physical> = geometry.to_physical_precise_round(scale);
        let buf_size = elem.buffer_size();
        let view = elem.view();

        let transform = elem.transform();
        let transform_matrix = Matrix3::<f32>::from_translation(Vector2::new(0.5, 0.5))
            * transform.matrix()
            * Matrix3::<f32>::from_translation(-Vector2::new(0.5, 0.5));

        let geo_scale = {
            let Scale { x, y } = elem_geo.size.to_f64() / geo.size.to_f64();
            Matrix3::from_nonuniform_scale(x as f32, y as f32)
        };

        let geo_translation = {
            let offset = (elem_geo.loc - geo.loc).to_f64();
            Matrix3::from_translation(Vector2::new(
                (offset.x / elem_geo.size.w as f64) as f32,
                (offset.y / elem_geo.size.h as f64) as f32,
            ))
        };

        let buf_scale = {
            let Scale { x, y } = buf_size.to_f64() / view.src.size;
            Matrix3::from_nonuniform_scale(x as f32, y as f32)
        };

        let buf_translation = Matrix3::from_translation(Vector2::new(
            (view.src.loc.x / buf_size.w as f64) as f32,
            (view.src.loc.y / buf_size.h as f64) as f32,
        ));

        let input_to_geo =
            transform_matrix * geo_scale * geo_translation * buf_scale * buf_translation;

        let uniforms = vec![
            Uniform::new("geo_size", (geometry.size.w as f32, geometry.size.h as f32)),
            Uniform::new(
                "corner_radius",
                [
                    radius[3] as f32,
                    radius[1] as f32,
                    radius[0] as f32,
                    radius[2] as f32,
                ],
            ),
            Uniform::new(
                "input_to_geo",
                UniformValue::Matrix3x3 {
                    matrices: vec![*AsRef::<[f32; 9]>::as_ref(&input_to_geo)],
                    transpose: false,
                },
            ),
        ];

        Self {
            inner: elem,
            program,
            radius,
            geometry,
            uniforms,
        }
    }

    fn rounded_corners(
        geo: Rectangle<f64, Logical>,
        radius: [u8; 4],
    ) -> [Rectangle<f64, Logical>; 4] {
        let top_left = radius[3] as f64;
        let top_right = radius[1] as f64;
        let bottom_right = radius[0] as f64;
        let bottom_left = radius[2] as f64;

        [
            Rectangle::new(geo.loc, Size::from((top_left, top_left))),
            Rectangle::new(
                Point::from((geo.loc.x + geo.size.w - top_right, geo.loc.y)),
                Size::from((top_right, top_right)),
            ),
            Rectangle::new(
                Point::from((
                    geo.loc.x + geo.size.w - bottom_right,
                    geo.loc.y + geo.size.h - bottom_right,
                )),
                Size::from((bottom_right, bottom_right)),
            ),
            Rectangle::new(
                Point::from((geo.loc.x, geo.loc.y + geo.size.h - bottom_left)),
                Size::from((bottom_left, bottom_left)),
            ),
        ]
    }
}

impl Element for ClippedSurfaceRenderElement {
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
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        let damage = self.inner.damage_since(scale, commit);
        let mut geo = self.geometry.to_physical_precise_round(scale);
        geo.loc -= self.geometry(scale).loc;
        damage
            .into_iter()
            .filter_map(|rect| rect.intersection(geo))
            .collect()
    }

    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        let regions = self.inner.opaque_regions(scale);

        let mut geo = self.geometry.to_physical_precise_round(scale);
        geo.loc -= self.geometry(scale).loc;
        let regions = regions
            .into_iter()
            .filter_map(|rect| rect.intersection(geo));

        let corners = Self::rounded_corners(self.geometry, self.radius);
        let elem_loc = self.geometry(scale).loc;
        let corners = corners.into_iter().map(|rect| {
            let mut rect = rect.to_physical_precise_up(scale);
            rect.loc -= elem_loc;
            rect
        });

        OpaqueRegions::from_slice(&Rectangle::subtract_rects_many(regions, corners))
    }

    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }

    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl RenderElement<GlesRenderer> for ClippedSurfaceRenderElement {
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
        let res = self
            .inner
            .draw(frame, src, dst, damage, opaque_regions, cache);
        frame.clear_tex_program_override();
        res
    }

    fn underlying_storage(&self, _renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        None
    }
}
