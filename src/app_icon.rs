//! The window icon, rasterized from the bundled SVG.

pub fn window_icon() -> Option<tao::window::Icon> {
    let tree = resvg::usvg::Tree::from_data(
        include_bytes!("../poof-icon.svg"),
        &resvg::usvg::Options::default(),
    )
    .ok()?;
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    tao::window::Icon::from_rgba(pixmap.data().to_vec(), size.width(), size.height()).ok()
}
