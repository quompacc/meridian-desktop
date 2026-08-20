use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            gles::{GlesRenderer, GlesTexture},
            Offscreen,
        },
    },
    utils::{Buffer, Size},
};

pub(crate) struct GlassPassBuffers {
    pub scene: GlesTexture,
    pub horizontal: GlesTexture,
    pub vertical: GlesTexture,
}

#[derive(Default)]
pub(crate) struct GlassPassCache {
    size: Option<(u32, u32)>,
    batches: Vec<GlassPassBuffers>,
}

impl GlassPassCache {
    pub fn batch(
        &mut self,
        renderer: &mut GlesRenderer,
        size: (u32, u32),
        index: usize,
    ) -> Option<&mut GlassPassBuffers> {
        if self.size != Some(size) {
            self.batches.clear();
            self.size = Some(size);
        }
        while self.batches.len() <= index {
            self.batches.push(create_buffers(renderer, size)?);
            tracing::debug!(
                width = size.0,
                height = size.1,
                batch = self.batches.len() - 1,
                "allocated reusable glass pass buffers"
            );
        }
        self.batches.get_mut(index)
    }
}

fn create_buffers(renderer: &mut GlesRenderer, size: (u32, u32)) -> Option<GlassPassBuffers> {
    let buffer_size = Size::<i32, Buffer>::from((size.0 as i32, size.1 as i32));
    let mut create = || {
        <GlesRenderer as Offscreen<GlesTexture>>::create_buffer(
            renderer,
            Fourcc::Abgr8888,
            buffer_size,
        )
        .ok()
    };
    Some(GlassPassBuffers {
        scene: create()?,
        horizontal: create()?,
        vertical: create()?,
    })
}
