//! The window and taskbar icons, rasterized from the bundled SVG.

/// The icon at `size` x `size` pixels, centered and keeping its proportions.
fn render(size: u32) -> Option<tao::window::Icon> {
    let tree = resvg::usvg::Tree::from_data(
        include_bytes!("../poof-icon.svg"),
        &resvg::usvg::Options::default(),
    )
    .ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    let (width, height) = (tree.size().width(), tree.size().height());
    let scale = (size as f32 / width).min(size as f32 / height);
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(
        (size as f32 - width * scale) / 2.0,
        (size as f32 - height * scale) / 2.0,
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    // tiny-skia stores premultiplied colors; icons want straight alpha.
    let mut rgba = pixmap.data().to_vec();
    for px in rgba.chunks_exact_mut(4) {
        let alpha = px[3] as u32;
        if alpha > 0 && alpha < 255 {
            for channel in &mut px[..3] {
                *channel = ((*channel as u32 * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    tao::window::Icon::from_rgba(rgba, size, size).ok()
}

/// Title bar / small icon.
pub fn window_icon() -> Option<tao::window::Icon> {
    render(32)
}

#[cfg_attr(not(windows), allow(dead_code))]
/// Taskbar / Alt+Tab icon: large, so Windows scales it down instead of up.
pub fn taskbar_icon() -> Option<tao::window::Icon> {
    render(256)
}
