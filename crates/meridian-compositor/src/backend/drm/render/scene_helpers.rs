fn themed_layer_glass_info(
    theme: &meridian_config::ThemeConfig,
    rect: smithay::utils::Rectangle<i32, smithay::utils::Logical>,
    surface: ThemeSurface,
) -> super::glass::GlassTitlebarInfo {
    let treatment = theme.decorations.surface_treatment(surface);
    let tint = theme.glass_tint_color();
    super::glass::GlassTitlebarInfo {
        rect,
        radius: [treatment.radius; 4],
        tint: [
            tint.r as f32 / 255.0,
            tint.g as f32 / 255.0,
            tint.b as f32 / 255.0,
        ],
        tint_amount: treatment.tint_amount,
        blur: treatment.blur_radius,
        // Per-surface fill opacity from the theme (glass_alpha), now the single
        // opacity knob for panel/launcher/popup instead of a shell-painted fill.
        fill_alpha: treatment.fill_alpha as f32 / 255.0,
    }
}

/// Render the scene behind one glass placeholder into an offscreen texture.
///
/// `first_behind` is an index into the front-to-back render list. Rendering only
/// elements after that index avoids sampling the glass surface itself or any UI
/// in front of it. This matters for the panel: its layer surface sits in front
/// of the compositor-owned backdrop, while windows/wallpaper sit behind it.
fn render_scene_for_blur(
    renderer: &mut GlesRenderer,
    elements: &[MeridianRenderElements],
    first_behind: usize,
    out_size: (u32, u32),
) -> Option<GlesTexture> {
    use smithay::backend::{
        allocator::Fourcc,
        renderer::{
            element::{Element, RenderElement},
            Bind, Frame as RendererFrame, Offscreen, Renderer,
        },
    };
    use smithay::utils::{Buffer, Physical, Rectangle, Scale, Size, Transform};

    let w = out_size.0 as i32;
    let h = out_size.1 as i32;
    let buf_size = Size::<i32, Buffer>::from((w, h));
    let phys_size = Size::<i32, Physical>::from((w, h));
    let phys_region = Rectangle::from_size(phys_size);

    let mut tex = <GlesRenderer as Offscreen<GlesTexture>>::create_buffer(
        renderer,
        Fourcc::Abgr8888,
        buf_size,
    )
    .ok()?;
    {
        let mut target = renderer.bind(&mut tex).ok()?;
        let mut frame = renderer
            .render(&mut target, phys_size, Transform::Normal)
            .ok()?;
        let _ = frame.clear([0.0, 0.0, 0.0, 1.0].into(), &[phys_region]);
        for element in elements.iter().skip(first_behind).rev() {
            if is_glass_blur_source_excluded(element) {
                continue;
            }
            let src = element.src();
            let dst = element.geometry(Scale::from(1.0f64));
            let dmg = [Rectangle::from_size(dst.size)];
            let _ = element.draw(&mut frame, src, dst, &dmg, &[], None);
        }
        drop(frame);
        drop(target);
    }
    Some(tex)
}

/// One separable blur pass: render `input` through the blur shader with the
/// given per-unit UV `step` into a fresh offscreen texture.
fn is_glass_blur_source_excluded(element: &MeridianRenderElements) -> bool {
    matches!(
        element,
        MeridianRenderElements::Decoration(_)
            | MeridianRenderElements::DecorationIcon(_)
            | MeridianRenderElements::Shadow(_)
            | MeridianRenderElements::Glass(_)
            | MeridianRenderElements::Cursor(_)
    )
}

fn first_rendered_blur_source(
    elements: &[MeridianRenderElements],
    mut first_behind: usize,
) -> usize {
    while matches!(elements.get(first_behind), Some(element) if is_glass_blur_source_excluded(element))
    {
        first_behind = first_behind.saturating_add(1);
    }
    first_behind
}

struct PendingGlassBatch {
    first_behind: usize,
    blur_bits: u32,
    items: Vec<(usize, super::glass::GlassTitlebarInfo)>,
}

fn blur_pass(
    renderer: &mut GlesRenderer,
    prog: &smithay::backend::renderer::gles::GlesTexProgram,
    input: GlesTexture,
    out_size: (u32, u32),
    step: (f32, f32),
) -> Option<GlesTexture> {
    use smithay::backend::{
        allocator::Fourcc,
        renderer::{
            element::{Element, RenderElement},
            gles::Uniform,
            Bind, Offscreen, Renderer,
        },
    };
    use smithay::utils::{Buffer, Logical, Physical, Point, Rectangle, Scale, Size, Transform};

    let w = out_size.0 as i32;
    let h = out_size.1 as i32;
    let buf_size = Size::<i32, Buffer>::from((w, h));
    let phys_size = Size::<i32, Physical>::from((w, h));

    let in_buf = TextureBuffer::from_texture(renderer, input, 1, Transform::Normal, None);
    let elem = TextureRenderElement::from_texture_buffer(
        Point::<f64, Physical>::from((0.0, 0.0)),
        &in_buf,
        Some(1.0),
        None::<Rectangle<f64, Logical>>,
        None::<Size<i32, Logical>>,
        Kind::Unspecified,
    );

    let mut out_tex = <GlesRenderer as Offscreen<GlesTexture>>::create_buffer(
        renderer,
        Fourcc::Abgr8888,
        buf_size,
    )
    .ok()?;
    {
        let mut target = renderer.bind(&mut out_tex).ok()?;
        let mut frame = renderer
            .render(&mut target, phys_size, Transform::Normal)
            .ok()?;
        frame.override_default_tex_program(prog.clone(), vec![Uniform::new("u_step", step)]);
        let dst = elem.geometry(Scale::from(1.0f64));
        let dmg = [Rectangle::from_size(dst.size)];
        let _ = RenderElement::<GlesRenderer>::draw(
            &elem,
            &mut frame,
            elem.src(),
            dst,
            &dmg,
            &[],
            None,
        );
        frame.clear_tex_program_override();
        drop(frame);
        drop(target);
    }
    Some(out_tex)
}

/// Two-pass separable Gaussian blur of the scene texture. Falls back to the
/// unblurred texture if the shader is unavailable or the radius is tiny.
fn blur_scene(
    renderer: &mut GlesRenderer,
    scene: GlesTexture,
    out_size: (u32, u32),
    radius: f32,
) -> Option<GlesTexture> {
    let prog = match super::glass::blur_shader(renderer) {
        Some(p) => p,
        None => return Some(scene),
    };
    if radius <= 0.5 {
        return Some(scene);
    }
    let spread = radius / 4.0;
    let ow = out_size.0.max(1) as f32;
    let oh = out_size.1.max(1) as f32;
    let tmp = blur_pass(renderer, &prog, scene, out_size, (spread / ow, 0.0))?;
    blur_pass(renderer, &prog, tmp, out_size, (0.0, spread / oh))
}

fn clear_output_dirty(
    output: &mut super::DrmOutput,
    dirty_stats: &mut super::DrmDirtyStats,
    reason: &str,
) {
    if output.needs_repaint {
        output.needs_repaint = false;
        dirty_stats.record_dirty_clear(output.output_id);
        tracing::trace!(
            "output repaint clean set: reason={} output_id={} output={}",
            reason,
            output.output_id.0,
            output.output.name()
        );
    }
}

fn render_window_popup_elements<C>(
    renderer: &mut GlesRenderer,
    window: &Window,
    window_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    scale: Scale<f64>,
    out: &mut Vec<C>,
) where
    C: From<SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>>,
{
    let WindowSurface::Wayland(toplevel) = window.underlying_surface() else {
        return;
    };

    let surface = toplevel.wl_surface();
    let location = window_loc.to_physical_precise_round(scale);
    out.extend(
        PopupManager::popups_for_surface(surface)
            .flat_map(|(popup, popup_offset)| {
                let offset = (window.geometry().loc + popup_offset - popup.geometry().loc)
                    .to_physical_precise_round(scale);
                render_elements_from_surface_tree::<
                    GlesRenderer,
                    WaylandSurfaceRenderElement<GlesRenderer>,
                >(
                    renderer,
                    popup.wl_surface(),
                    location + offset,
                    scale,
                    1.0,
                    Kind::Unspecified,
                )
            })
            .map(SpaceRenderElements::from)
            .map(C::from),
    );
}
