//! Wayland surface wrapper for clients whose advertised opaque region is not
//! valid for every pixel they submit.
//!
//! WebKitGTK's transparent layer-shell host advertises the launcher window as
//! opaque even though its CSS shadow margin contains alpha. Trusting that hint
//! lets partial damage blend a new shadow over the previous framebuffer. This
//! wrapper preserves rendering and damage while disabling opaque culling.

use smithay::{
    backend::renderer::{
        element::{
            surface::WaylandSurfaceRenderElement, Element, Id, Kind, RenderElement,
            UnderlyingStorage,
        },
        gles::{GlesError, GlesFrame, GlesRenderer},
        utils::{CommitCounter, DamageSet, OpaqueRegions},
    },
    utils::{user_data::UserDataMap, Buffer, Physical, Rectangle, Scale, Transform},
};

#[derive(Debug)]
pub struct NonOpaqueSurfaceRenderElement {
    inner: WaylandSurfaceRenderElement<GlesRenderer>,
}

impl NonOpaqueSurfaceRenderElement {
    pub fn new(inner: WaylandSurfaceRenderElement<GlesRenderer>) -> Self {
        Self { inner }
    }
}

impl Element for NonOpaqueSurfaceRenderElement {
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
        self.inner.damage_since(scale, commit)
    }

    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        OpaqueRegions::default()
    }

    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }

    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl RenderElement<GlesRenderer> for NonOpaqueSurfaceRenderElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
        cache: Option<&UserDataMap>,
    ) -> Result<(), GlesError> {
        self.inner
            .draw(frame, src, dst, damage, opaque_regions, cache)
    }

    fn underlying_storage(&self, renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        self.inner.underlying_storage(renderer)
    }
}
