//! Conversions between window pixels and wry rectangles.

use super::*;

pub(super) fn logical(w: &Window) -> (f64, f64) {
    let s: tao::dpi::LogicalSize<f64> = w.inner_size().to_logical(w.scale_factor());
    (s.width, s.height)
}

pub(super) fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(w.max(1.0), h.max(1.0)).into(),
    }
}

pub(super) fn rect_of(b: [f64; 4]) -> Rect {
    rect(b[0], b[1], b[2], b[3])
}

pub(super) fn full_rect(w: &Window) -> Rect {
    let (ww, hh) = logical(w);
    rect(0.0, 0.0, ww, hh)
}
