//! Colour picking: a hue slider, a saturation and lightness pad, and a grid
//! of preset swatches.
//!
//! Values are OKLCH throughout, matching [`crate::Palette`]. Hue is degrees,
//! chroma 0..=[`MAX_CHROMA`], lightness 0..=1. A pad reports positions the
//! same way a slider does, through [`ControlHost::track_dragged`], with `x`
//! for chroma and `y` for lightness.

use gpui::{
    AnyElement, Context, ElementId, MouseButton, MouseDownEvent, Window, div, prelude::*, px,
};

use crate::color::oklch_to_color;
use crate::controls::{SliderTrack, slider, track_probe};
use crate::lighting;
use crate::palette::Palette;
use crate::state::{ComboId, ControlHost, TrackAxis};

/// Chroma beyond this leaves the sRGB gamut for most hues and lightnesses,
/// so the pad would have a dead region along its right edge.
pub const MAX_CHROMA: f64 = 0.16;

/// An OKLCH colour, as the pickers here pass it around.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Oklch {
    /// Degrees, 0..360.
    pub hue: f64,
    /// 0..=[`MAX_CHROMA`].
    pub chroma: f64,
    /// 0..=1.
    pub lightness: f64,
}

impl Oklch {
    pub fn new(hue: f64, chroma: f64, lightness: f64) -> Self {
        Self {
            hue: hue.rem_euclid(360.0),
            chroma: chroma.clamp(0.0, MAX_CHROMA),
            lightness: lightness.clamp(0.0, 1.0),
        }
    }

    pub fn to_rgba(self) -> gpui::Rgba {
        oklch_to_color(self.lightness, self.chroma, self.hue)
    }
}

/// Hue slider: the same control as [`slider`], labelled for what it does.
///
/// `hue` is in degrees; the drag arrives at
/// [`ControlHost::track_dragged`] as a 0..=1 fraction of the full turn, so
/// multiply by 360.
pub fn hue_slider<V: ControlHost>(
    id: ComboId,
    hue: f64,
    palette: Palette,
    cx: &mut Context<V>,
) -> impl IntoElement {
    slider(
        id,
        (hue.rem_euclid(360.0) / 360.0) as f32,
        SliderTrack::Continuous,
        palette,
        cx,
    )
}

/// Saturation and lightness pad for one hue: chroma left to right, lightness
/// bottom to top, with a ring on the current colour.
///
/// Painted as a grid of cells rather than a true gradient, because gpui has
/// no two-dimensional gradient. At this size the seams do not read, and a
/// pad small enough to matter is small enough to be cheap.
pub fn color_pad<V: ControlHost>(
    id: ComboId,
    color: Oklch,
    height: f32,
    palette: Palette,
    cx: &mut Context<V>,
) -> impl IntoElement {
    const COLUMNS: usize = 24;
    const ROWS: usize = 16;

    let weak = cx.entity().downgrade();
    let hue = color.hue;

    let mut bands: Vec<AnyElement> = Vec::with_capacity(ROWS);
    for row in 0..ROWS {
        // Row 0 is the top, which is the lightest.
        let lightness = 1.0 - (row as f64 + 0.5) / ROWS as f64;
        let mut cells: Vec<AnyElement> = Vec::with_capacity(COLUMNS);
        for column in 0..COLUMNS {
            let chroma = (column as f64 + 0.5) / COLUMNS as f64 * MAX_CHROMA;
            cells.push(
                div()
                    .flex_1()
                    .h_full()
                    .bg(oklch_to_color(lightness, chroma, hue))
                    .into_any_element(),
            );
        }
        bands.push(
            div()
                .flex_1()
                .w_full()
                .flex()
                .children(cells)
                .into_any_element(),
        );
    }

    let marker_x = (color.chroma / MAX_CHROMA) as f32;
    let marker_y = 1.0 - color.lightness as f32;

    div()
        .id(id)
        .h(px(height))
        .w_full()
        .relative()
        .overflow_hidden()
        .rounded(px(6.0))
        .border_1()
        .border_color(palette.field_border)
        .shadow(lighting::recessed(palette.is_dark))
        .cursor_crosshair()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                this.control_state_mut()
                    .begin_track_drag(id, TrackAxis::Both, None);
                if let Some((_, at)) = this.control_state().track_ratio_at(event.position) {
                    this.track_dragged(id, at, cx);
                }
                cx.notify();
            }),
        )
        .child(track_probe::<V>(id, weak))
        .child(div().absolute().inset_0().flex().flex_col().children(bands))
        .child(
            // A white ring with a dark inner edge, so the marker stays
            // visible over both ends of the pad.
            div()
                .absolute()
                .left(gpui::relative(marker_x))
                .top(gpui::relative(marker_y))
                .ml(px(-6.0))
                .mt(px(-6.0))
                .w(px(12.0))
                .h(px(12.0))
                .rounded_full()
                .border_2()
                .border_color(gpui::white())
                .shadow(vec![lighting::glow(
                    gpui::Rgba {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    0.45,
                    2.0,
                )]),
        )
}

/// Grid of preset swatches, with a ring on whichever matches `selected`.
///
/// Presets are how most people actually pick a colour, so put this above the
/// pad rather than beside it.
pub fn swatch_grid<V: ControlHost>(
    id: &'static str,
    swatches: &[Oklch],
    selected: Option<Oklch>,
    size: f32,
    palette: Palette,
    cx: &mut Context<V>,
    on_pick: impl Fn(&mut V, Oklch, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let on_pick = std::rc::Rc::new(on_pick);
    let mut cells: Vec<AnyElement> = Vec::with_capacity(swatches.len());

    for (index, swatch) in swatches.iter().enumerate() {
        let cx: &mut Context<V> = &mut *cx;
        let on_pick = on_pick.clone();
        let swatch = *swatch;
        // Compared on the values rather than the resulting colour: two
        // OKLCH triples can round to the same sRGB and still be different
        // picks, and the swatch the person clicked is the one to ring.
        let active = selected.is_some_and(|current| {
            (current.hue - swatch.hue).abs() < 0.5
                && (current.chroma - swatch.chroma).abs() < 0.005
                && (current.lightness - swatch.lightness).abs() < 0.005
        });
        cells.push(
            div()
                .id(ElementId::NamedInteger(
                    format!("{id}-swatch").into(),
                    index as u64,
                ))
                .w(px(size))
                .h(px(size))
                .flex_none()
                .rounded(px(5.0))
                .bg(swatch.to_rgba())
                .border_2()
                .border_color(if active {
                    palette.accent
                } else {
                    lighting::rim(swatch.to_rgba(), palette.is_dark)
                })
                .shadow(lighting::raised(palette.is_dark))
                .cursor_pointer()
                .on_click(cx.listener(move |this, _event, window, cx| {
                    on_pick(this, swatch, window, cx);
                    cx.notify();
                }))
                .into_any_element(),
        );
    }

    div().flex().flex_wrap().gap(px(6.0)).children(cells)
}

/// Twelve evenly spaced hues at one lightness and chroma: a reasonable
/// default row for [`swatch_grid`].
pub fn hue_wheel(lightness: f64, chroma: f64) -> Vec<Oklch> {
    (0..12)
        .map(|step| Oklch::new(step as f64 * 30.0, chroma, lightness))
        .collect()
}
