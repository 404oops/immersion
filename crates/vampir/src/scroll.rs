//! Overlay-scrollbar geometry and thumb dragging.
//!
//! Split out from the drawing so a host can keep a drag alive from its own
//! root mouse handlers: the pointer leaves the thin track almost at once,
//! and every move after that arrives somewhere else entirely.

use gpui::{ScrollHandle, point, px};

/// Shortest a thumb is allowed to get, however long the content is.
pub const SCROLLBAR_MIN_THUMB: f32 = 24.0;
/// Width of the vertical track, height of the horizontal one.
pub const SCROLLBAR_THICKNESS: f32 = 10.0;
/// The visible bar inside that track.
pub const THUMB_THICKNESS: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
}

impl ScrollAxis {
    pub fn other(self) -> Self {
        match self {
            ScrollAxis::Vertical => ScrollAxis::Horizontal,
            ScrollAxis::Horizontal => ScrollAxis::Vertical,
        }
    }
}

/// A thumb drag in flight. The host stores it and feeds pointer positions to
/// [`apply_scroll_drag`] until the button comes up.
pub struct ScrollDrag {
    pub handle: ScrollHandle,
    pub axis: ScrollAxis,
    /// Where inside the thumb the pointer grabbed it, in px. Without this
    /// the thumb would jump so its start sat under the cursor.
    pub grab: f32,
}

pub struct ScrollbarGeometry {
    pub track_start: f32,
    /// Usable track: the viewport minus the corner reserved for the other
    /// axis's bar.
    pub track_len: f32,
    pub track_end_inset: f32,
    pub thumb_len: f32,
    pub thumb_pos: f32,
    pub max_offset: f32,
}

/// (track start, viewport length, max offset, current offset) along `axis`.
fn axis_metrics(handle: &ScrollHandle, axis: ScrollAxis) -> (f32, f32, f32, f32) {
    let bounds = handle.bounds();
    match axis {
        ScrollAxis::Vertical => (
            f32::from(bounds.top()),
            f32::from(bounds.size.height),
            f32::from(handle.max_offset().y),
            f32::from(handle.offset().y),
        ),
        ScrollAxis::Horizontal => (
            f32::from(bounds.left()),
            f32::from(bounds.size.width),
            f32::from(handle.max_offset().x),
            f32::from(handle.offset().x),
        ),
    }
}

fn axis_scrollable(handle: &ScrollHandle, axis: ScrollAxis) -> bool {
    let (_, viewport_len, max_offset, _) = axis_metrics(handle, axis);
    max_offset > 0.5 && viewport_len > 0.0
}

/// `None` when the content fits and no bar should be drawn.
pub fn scrollbar_geometry(handle: &ScrollHandle, axis: ScrollAxis) -> Option<ScrollbarGeometry> {
    let (track_start, viewport_len, max_offset, offset) = axis_metrics(handle, axis);
    if max_offset <= 0.5 || viewport_len <= 0.0 {
        return None;
    }
    // When both bars are on screen, stop each one short of the shared corner.
    // Otherwise the two tracks overlap there and whichever is painted last
    // swallows the other's drags.
    let track_end_inset = if axis_scrollable(handle, axis.other()) {
        SCROLLBAR_THICKNESS
    } else {
        0.0
    };
    let track_len = (viewport_len - track_end_inset).max(1.0);
    let content_len = viewport_len + max_offset;
    let thumb_len = (track_len * viewport_len / content_len)
        .max(SCROLLBAR_MIN_THUMB)
        .min(track_len);
    let usable = (track_len - thumb_len).max(0.0);
    let fraction = (-offset / max_offset).clamp(0.0, 1.0);
    Some(ScrollbarGeometry {
        track_start,
        track_len,
        track_end_inset,
        thumb_len,
        thumb_pos: fraction * usable,
        max_offset,
    })
}

/// Scrolls the handle to match a pointer position, in window coordinates
/// along the drag's axis.
pub fn apply_scroll_drag(drag: &ScrollDrag, position: f32) {
    let Some(geometry) = scrollbar_geometry(&drag.handle, drag.axis) else {
        return;
    };
    let usable = (geometry.track_len - geometry.thumb_len).max(1.0);
    let thumb_pos = (position - geometry.track_start - drag.grab).clamp(0.0, usable);
    let fraction = thumb_pos / usable;
    let target = -fraction * geometry.max_offset;
    let current = drag.handle.offset();
    match drag.axis {
        ScrollAxis::Vertical => drag.handle.set_offset(point(current.x, px(target))),
        ScrollAxis::Horizontal => drag.handle.set_offset(point(px(target), current.y)),
    }
}
