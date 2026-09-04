//! Themed controls: action and panel buttons, toggle switch, hue slider,
//! spin box, combo box, overlay scrollbar, easing helpers, and the
//! text-field/area chrome around TextInput.

use std::time::{Duration, Instant};

use gpui::{
    Context, ElementId, Entity, FontWeight, MouseButton, PathBuilder, Rgba, ScrollHandle,
    SharedString, Window, canvas, deferred, div, point, prelude::*, px,
};

use crate::app::{ComboId, RootView};
use crate::text_input::TextInput;
use crate::theme::Theme;
use crate::ui::lighting;

/// Shared metrics: every field, combo and button in the app is this tall
/// and this round, so rows of mixed controls line up without tuning.
pub const CONTROL_HEIGHT: f32 = 30.0;
pub const CONTROL_RADIUS: f32 = 6.0;
/// A pop-up list fades and slides into place over this long.
pub const COMBO_REVEAL: Duration = Duration::from_millis(140);

/// Section label: a quiet, medium-weight line that names a region without
/// competing with the content in it.
pub fn caption(theme: &Theme, text: &str) -> impl IntoElement {
    div()
        .flex_none()
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.text_secondary)
        .whitespace_nowrap()
        .child(SharedString::from(text.to_string()))
}

// ---- Overlay scrollbars -----------------------------------------------------

pub const SCROLLBAR_MIN_THUMB: f32 = 24.0;
/// Width of the vertical track / height of the horizontal one.
const SCROLLBAR_THICKNESS: f32 = 10.0;
/// The visible bar inside that track.
const THUMB_THICKNESS: f32 = 4.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
}

impl ScrollAxis {
    fn other(self) -> Self {
        match self {
            ScrollAxis::Vertical => ScrollAxis::Horizontal,
            ScrollAxis::Horizontal => ScrollAxis::Vertical,
        }
    }
}

/// An in-flight scrollbar thumb drag; lives on RootView so the root-level
/// mouse handlers can keep tracking outside the thumb.
pub struct ScrollDrag {
    pub handle: ScrollHandle,
    pub axis: ScrollAxis,
    /// Pointer offset within the thumb at drag start, px.
    pub grab: f32,
}

struct ScrollbarGeometry {
    track_start: f32,
    /// Length of the usable track: the viewport minus the corner reserved for
    /// the other axis's bar.
    track_len: f32,
    track_end_inset: f32,
    thumb_len: f32,
    thumb_pos: f32,
    max_offset: f32,
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

fn scrollbar_geometry(handle: &ScrollHandle, axis: ScrollAxis) -> Option<ScrollbarGeometry> {
    let (track_start, viewport_len, max_offset, offset) = axis_metrics(handle, axis);
    if max_offset <= 0.5 || viewport_len <= 0.0 {
        return None;
    }
    // When both bars are on screen, stop each one short of the shared corner —
    // otherwise the two tracks overlap there and the one painted last swallows
    // the other's drags.
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

/// Applies a drag position (window coords along the axis) to the handle.
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

/// Interactive overlay scrollbar: draggable thumb, click on
/// the track jumps. Add inside a `.relative()` wrapper around the scroll
/// container; renders nothing while the content fits.
pub fn scrollbar(
    id: &'static str,
    handle: &ScrollHandle,
    axis: ScrollAxis,
    theme: &Theme,
    cx: &mut Context<RootView>,
) -> gpui::AnyElement {
    let Some(geometry) = scrollbar_geometry(handle, axis) else {
        return div().absolute().into_any_element();
    };
    // Faint by default: an overlay bar is a hint about position, not a
    // control competing with the content. It firms up under the pointer.
    let mut thumb_color: gpui::Hsla = gpui::Rgba::from(theme.text_secondary).into();
    thumb_color.a = 0.24;

    let drag_handle = handle.clone();
    let track = div()
        .id(ElementId::Name(format!("scrollbar-{id}").into()))
        .absolute()
        // Block clicks/hover from bleeding into rows under the track, but let
        // wheel deltas pass through so the strip isn't a scroll dead zone.
        .block_mouse_except_scroll()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &gpui::MouseDownEvent, _window, cx| {
                let Some(geometry) = scrollbar_geometry(&drag_handle, axis) else {
                    return;
                };
                let position = match axis {
                    ScrollAxis::Vertical => f32::from(event.position.y),
                    ScrollAxis::Horizontal => f32::from(event.position.x),
                };
                let within = position - geometry.track_start - geometry.thumb_pos;
                let grab = if (0.0..=geometry.thumb_len).contains(&within) {
                    within
                } else {
                    // Track click: center the thumb on the pointer.
                    geometry.thumb_len / 2.0
                };
                let drag = ScrollDrag {
                    handle: drag_handle.clone(),
                    axis,
                    grab,
                };
                apply_scroll_drag(&drag, position);
                this.scroll_drag = Some(drag);
                cx.notify();
            }),
        )
        // Blocking the mouse also truncates hover at this strip: while the
        // pointer stays inside it the root is not hovered, so the root's
        // drag tracking sees neither the moves nor the release. Forward both
        // the way the occluding modal surfaces do.
        .on_mouse_move(
            cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                this.global_mouse_move(event, cx);
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _event, _window, cx| {
                this.global_mouse_up(cx);
            }),
        );

    let thumb = div()
        .absolute()
        .rounded_full()
        .bg(thumb_color)
        .hover(move |style| {
            let mut hovered = thumb_color;
            hovered.a = 0.50;
            style.bg(hovered)
        });

    match axis {
        ScrollAxis::Vertical => track
            .top_0()
            .bottom(px(geometry.track_end_inset))
            .right_0()
            .w(px(SCROLLBAR_THICKNESS))
            .child(
                thumb
                    .top(px(geometry.thumb_pos))
                    .right(px(3.0))
                    .w(px(THUMB_THICKNESS))
                    .h(px(geometry.thumb_len)),
            )
            .into_any_element(),
        ScrollAxis::Horizontal => track
            .left_0()
            .right(px(geometry.track_end_inset))
            .bottom_0()
            .h(px(SCROLLBAR_THICKNESS))
            .child(
                thumb
                    .left(px(geometry.thumb_pos))
                    .bottom(px(3.0))
                    .h(px(THUMB_THICKNESS))
                    .w(px(geometry.thumb_len)),
            )
            .into_any_element(),
    }
}

/// Cubic ease-out: fast start, decelerating into the end value.
pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Cubic ease-in: slow start, accelerating into the end value (popup exit,
/// etc.).
pub fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
}

pub fn lerp_rgba(from: Rgba, to: Rgba, t: f32) -> Rgba {
    let mix = |a: f32, b: f32| a + (b - a) * t;
    Rgba {
        r: mix(from.r, to.r),
        g: mix(from.g, to.g),
        b: mix(from.b, to.b),
        a: mix(from.a, to.a),
    }
}

pub fn lerp_f32(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// Returns `(opacity, still_animating)` for popup enter/exit fades.
pub fn modal_opacity(enter_at: Option<Instant>, exit_at: Option<Instant>) -> (f32, bool) {
    use crate::theme::{MODAL_ENTER_DURATION_MS, MODAL_EXIT_DURATION_MS};

    if let Some(since) = exit_at {
        let duration = Duration::from_millis(MODAL_EXIT_DURATION_MS);
        let t = (since.elapsed().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
        (1.0 - ease_in_cubic(t), t < 1.0)
    } else if let Some(since) = enter_at {
        let duration = Duration::from_millis(MODAL_ENTER_DURATION_MS);
        let t = (since.elapsed().as_secs_f32() / duration.as_secs_f32()).clamp(0.0, 1.0);
        (ease_out_cubic(t), t < 1.0)
    } else {
        (1.0, false)
    }
}

/// Instant-driven hover fill helpers were removed: scrolling re-fires hover on
/// every row and pinned the display link at 60Hz. Buttons use GPUI `.hover()`.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Soft,
    Primary,
    Danger,
}

type ClickHandler = Box<dyn Fn(&mut RootView, &mut Window, &mut Context<RootView>) + 'static>;

/// Button lit from above: gradient fill, a highlight along the top edge, a
/// darker rim and a soft shadow. Soft is the tinted secondary button,
/// Primary the single call to action in a group, Danger a tinted red.
pub fn panel_button(
    id: impl Into<ElementId>,
    text: &str,
    variant: ButtonVariant,
    enabled: bool,
    theme: &Theme,
    _view: &RootView,
    cx: &mut Context<RootView>,
    on_click: impl Fn(&mut RootView, &mut Window, &mut Context<RootView>) + 'static,
) -> impl IntoElement {
    let theme = *theme;
    let dark = theme.is_dark_mode;
    let (fill, fill_hover, label_color) = match variant {
        ButtonVariant::Primary => (
            theme.button_primary_fill,
            theme.button_primary_fill_hover,
            theme.button_primary_label,
        ),
        ButtonVariant::Danger => (
            theme.button_danger_fill,
            theme.button_danger_fill_hover,
            theme.button_danger_label,
        ),
        ButtonVariant::Soft => (
            theme.button_soft_fill,
            theme.button_soft_fill_hover,
            theme.button_soft_label,
        ),
    };
    let label: SharedString = text.to_string().into();
    let on_click: ClickHandler = Box::new(on_click);

    div()
        .id(id.into())
        .h(px(CONTROL_HEIGHT))
        .w_full()
        .px(px(12.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(lighting::lit(fill, 0.08))
        .border_1()
        .border_color(lighting::rim(fill, dark))
        .shadow(lighting::raised(dark))
        .text_size(px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(label_color)
        .whitespace_nowrap()
        .overflow_hidden()
        .when(!enabled, |el| el.opacity(0.5))
        .when(enabled, move |el| {
            el.cursor_pointer()
                .hover(move |style| style.bg(lighting::lit(fill_hover, 0.1)))
                .active(move |style| style.bg(lighting::lit(lighting::shade(fill, -0.06), 0.0)))
                .on_click(cx.listener(move |this, _event, window, cx| {
                    on_click(this, window, cx);
                }))
        })
        .child(label)
}

/// Toggle switch.
///
/// The track colour and thumb position animate over 140ms with
/// [`ease_out_cubic`]. `switch_anim` records the toggle instant so the first
/// paint (and remounts) stay still — only real toggles slide.
pub fn themed_switch(
    id: impl Into<ElementId>,
    checked: bool,
    enabled: bool,
    theme: &Theme,
    view: &RootView,
    cx: &mut Context<RootView>,
    on_toggle: impl Fn(&mut RootView, bool, &mut Window, &mut Context<RootView>) + 'static,
) -> impl IntoElement {
    const SLIDE: Duration = Duration::from_millis(140);
    const KNOB: f32 = 20.0;
    const TRACK_W: f32 = 46.0;
    const PAD: f32 = 3.0;
    let left_off = PAD;
    let left_on = TRACK_W - KNOB - PAD;

    let theme = *theme;
    let id: ElementId = id.into();
    let track_off = theme.input_border;
    let track_on = theme.accent;
    let track_target = if checked { track_on } else { track_off };
    let track_source = if checked { track_off } else { track_on };
    let left_target = if checked { left_on } else { left_off };
    let left_source = if checked { left_off } else { left_on };
    let track_opacity = if !enabled {
        0.45
    } else if checked {
        if theme.is_dark_mode { 0.5 } else { 0.58 }
    } else {
        1.0
    };

    let t = view
        .switch_anim
        .get(&id)
        .map(|since| (since.elapsed().as_secs_f32() / SLIDE.as_secs_f32()).clamp(0.0, 1.0))
        .unwrap_or(1.0);
    let eased = ease_out_cubic(t);
    let track_color = lerp_rgba(track_source, track_target, eased);
    let left = lerp_f32(left_source, left_target, eased);

    let anim_key = id.clone();
    div()
        .id(id)
        .w(px(TRACK_W))
        .h(px(26.0))
        .flex_none()
        .relative()
        .when(enabled, |el| {
            el.cursor_pointer()
                .on_click(cx.listener(move |this, _event, window, cx| {
                    this.switch_anim.insert(anim_key.clone(), Instant::now());
                    on_toggle(this, !checked, window, cx);
                    cx.notify();
                }))
        })
        .child(
            div()
                .absolute()
                .inset_0()
                .rounded(px(13.0))
                .bg(track_color)
                .opacity(track_opacity),
        )
        .child(
            div()
                .absolute()
                .top(px(PAD))
                .left(px(left))
                .w(px(KNOB))
                .h(px(KNOB))
                .rounded_full()
                .bg(theme.input_surface)
                .border_1()
                .border_color(theme.input_border_accent),
        )
}

/// Spin box: editable value field + increment/decrement buttons.
pub fn themed_spinbox(
    id_prefix: &'static str,
    value: i32,
    min: i32,
    max: i32,
    enabled: bool,
    edit_input: &Entity<TextInput>,
    theme: &Theme,
    cx: &mut Context<RootView>,
    on_change: impl Fn(&mut RootView, i32, &mut Window, &mut Context<RootView>) + Clone + 'static,
) -> impl IntoElement {
    let theme = *theme;
    let dec = on_change.clone();
    let inc = on_change;
    let step_button = |id: ElementId,
                       glyph: &'static str,
                       active: bool,
                       cx: &mut Context<RootView>,
                       handler: ClickHandler| {
        div()
            .id(id)
            .w(px(22.0))
            .h(px(22.0))
            .rounded(px(3.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .text_color(theme.text_secondary)
            .when(!enabled, |el| el.opacity(0.45))
            .when(enabled && active, move |el| {
                el.cursor_pointer()
                    .hover(move |style| style.bg(theme.button_soft_fill))
                    .active(move |style| style.bg(theme.button_soft_fill_hover))
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        handler(this, window, cx);
                    }))
            })
            .child(glyph)
    };

    div()
        .w(px(118.0))
        .h(px(CONTROL_HEIGHT))
        .flex_none()
        .rounded(px(CONTROL_RADIUS))
        .bg(theme.input_surface)
        .border_1()
        .border_color(theme.input_border)
        .shadow(lighting::recessed(theme.is_dark_mode))
        .when(!enabled, |el| el.opacity(0.5))
        .flex()
        .items_center()
        .justify_between()
        .px(px(4.0))
        .child(step_button(
            ElementId::Name(format!("{id_prefix}-dec").into()),
            "\u{2212}",
            value > min,
            cx,
            Box::new(move |this, window, cx| {
                dec(this, (value - 1).max(min), window, cx);
            }),
        ))
        .child(
            // Direct entry: the value field is an editable text input.
            div()
                .flex_1()
                .px(px(4.0))
                .text_size(px(12.0))
                .text_color(theme.text_primary)
                .child(edit_input.clone()),
        )
        .child(step_button(
            ElementId::Name(format!("{id_prefix}-inc").into()),
            "+",
            value < max,
            cx,
            Box::new(move |this, window, cx| {
                inc(this, (value + 1).min(max), window, cx);
            }),
        ))
}

/// Single-line text field: a recessed white well with a focus glow.
pub fn text_field(
    input: &Entity<TextInput>,
    theme: &Theme,
    window: &Window,
    cx: &Context<RootView>,
) -> impl IntoElement {
    let focused = input.read(cx).focus_handle.is_focused(window);
    let theme = *theme;
    let click_input = input.clone();
    div()
        .h(px(CONTROL_HEIGHT))
        .w_full()
        .px(px(9.0))
        .flex()
        .items_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(theme.input_surface)
        .border_1()
        .border_color(if focused {
            theme.accent
        } else {
            theme.input_border
        })
        .shadow(well_shadows(&theme, focused))
        .overflow_hidden()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_root, event: &gpui::MouseDownEvent, window, cx| {
                click_input.update(cx, |input, cx| {
                    input.handle_chrome_click(event.position, window, cx)
                });
            }),
        )
        .child(input.clone())
}

/// Recessed well, plus the accent focus glow while focused.
fn well_shadows(theme: &Theme, focused: bool) -> Vec<gpui::BoxShadow> {
    let mut shadows = lighting::recessed(theme.is_dark_mode);
    if focused {
        shadows.push(lighting::glow(theme.accent, 0.35, 3.0));
    }
    shadows
}

/// Text area: the same recessed well as the field, filling its flex slot
/// (min 40px) when `height` is None so the details panel scales with the
/// divider instead of clipping.
pub fn text_area(
    input: &Entity<TextInput>,
    height: Option<f32>,
    enabled: bool,
    theme: &Theme,
    window: &Window,
    cx: &Context<RootView>,
) -> impl IntoElement {
    let focused = input.read(cx).focus_handle.is_focused(window);
    let theme = *theme;
    let click_input = input.clone();
    div()
        .id(ElementId::Name(
            format!("text-area-{}", input.entity_id()).into(),
        ))
        .when_some(height, |el, h| el.h(px(h)))
        .when(height.is_none(), |el| el.flex_1().min_h(px(40.0)))
        .line_height(px(17.0))
        .w_full()
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(CONTROL_RADIUS))
        .bg(theme.vm_input_surface)
        .border_1()
        .border_color(if focused {
            theme.accent
        } else {
            theme.vm_border
        })
        .shadow(well_shadows(&theme, focused))
        .when(!enabled, |el| el.opacity(0.5))
        .overflow_y_scroll()
        .when(enabled, |el| {
            el.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_root, event: &gpui::MouseDownEvent, window, cx| {
                    click_input.update(cx, |input, cx| {
                        input.handle_chrome_click(event.position, window, cx)
                    });
                }),
            )
        })
        .child(input.clone())
}

impl RootView {
    /// Pop-up button: a raised control with the accent chevron chip on the
    /// right, and a floating list when open.
    pub fn render_combo(
        &self,
        combo: ComboId,
        id: &'static str,
        current_index: usize,
        options: &[String],
        width: Option<f32>,
        cx: &mut Context<RootView>,
        on_select: impl Fn(&mut RootView, usize, &mut Window, &mut Context<RootView>) + 'static,
    ) -> impl IntoElement {
        let theme = self.theme;
        let dark = theme.is_dark_mode;
        let is_open = self.open_combo == Some(combo);
        // The list keeps rendering while it fades back out after a close.
        let closing_since = self.combo_closing.and_then(|(closing, since)| {
            (closing == combo && since.elapsed() < COMBO_REVEAL).then_some(since)
        });
        let showing = is_open || closing_since.is_some();
        // Reveal progress of the list: opacity, and a short slide between the
        // control and its resting place. Opening eases out, closing eases in,
        // so both ends of the motion sit against the control.
        let reveal = if is_open {
            self.combo_opened_at
                .map(|since| {
                    ease_out_cubic(
                        (since.elapsed().as_secs_f32() / COMBO_REVEAL.as_secs_f32())
                            .clamp(0.0, 1.0),
                    )
                })
                .unwrap_or(1.0)
        } else if let Some(since) = closing_since {
            1.0 - ease_in_cubic(
                (since.elapsed().as_secs_f32() / COMBO_REVEAL.as_secs_f32()).clamp(0.0, 1.0),
            )
        } else {
            1.0
        };
        let slide = 6.0 * (1.0 - reveal);
        // The main-file picker lives at the bottom of the details panel, a
        // few pixels above the window edge: its list opens upward so it is
        // never cut off. Everything else has room below.
        let opens_upward = matches!(combo, ComboId::PrimaryFile);
        let display: SharedString = options
            .get(current_index)
            .cloned()
            .unwrap_or_default()
            .into();
        let options_owned: Vec<String> = options.to_vec();
        let on_select = std::rc::Rc::new(on_select);
        let body_fill = if dark {
            theme.button_soft_fill
        } else {
            theme.input_surface
        };
        let chip_fill = theme.button_fill;
        let chevron_color: gpui::Hsla = theme.button_label.into();

        div()
            .relative()
            .when_some(width, |el, w| el.w(px(w)).flex_none())
            .when(width.is_none(), |el| el.w_full())
            .child(
                div()
                    .id(ElementId::Name(format!("{id}-toggle").into()))
                    .h(px(CONTROL_HEIGHT))
                    .w_full()
                    .pl(px(10.0))
                    .pr(px(28.0))
                    .flex()
                    .items_center()
                    .rounded(px(CONTROL_RADIUS))
                    .bg(lighting::lit(body_fill, 0.05))
                    .border_1()
                    .border_color(if is_open {
                        theme.accent
                    } else {
                        lighting::rim(body_fill, dark)
                    })
                    .shadow(lighting::raised(dark))
                    .cursor_pointer()
                    .text_size(px(12.5))
                    .text_color(theme.text_primary)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .hover(move |style| style.bg(lighting::lit(body_fill, 0.09)))
                    .child(display)
                    // Chevron chip: accent square with up/down arrows.
                    .child(
                        div()
                            .absolute()
                            .right(px(5.0))
                            .top(px((CONTROL_HEIGHT - 2.0 - 18.0) / 2.0))
                            .w(px(18.0))
                            .h(px(18.0))
                            .rounded(px(CONTROL_RADIUS - 2.0))
                            .bg(lighting::lit(chip_fill, 0.12))
                            .shadow(lighting::raised(dark))
                            .child(
                                canvas(
                                    |_bounds, _window, _cx| {},
                                    move |bounds, _state, window, _cx| {
                                        let o = bounds.origin;
                                        let mut builder = PathBuilder::stroke(px(1.5));
                                        // Up chevron.
                                        builder.move_to(point(o.x + px(5.5), o.y + px(7.5)));
                                        builder.line_to(point(o.x + px(9.0), o.y + px(4.0)));
                                        builder.line_to(point(o.x + px(12.5), o.y + px(7.5)));
                                        // Down chevron.
                                        builder.move_to(point(o.x + px(5.5), o.y + px(10.5)));
                                        builder.line_to(point(o.x + px(9.0), o.y + px(14.0)));
                                        builder.line_to(point(o.x + px(12.5), o.y + px(10.5)));
                                        if let Ok(path) = builder.build() {
                                            window.paint_path(path, chevron_color);
                                        }
                                    },
                                )
                                .size_full(),
                            ),
                    )
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        // The popup's mouse_down_out already closed the combo
                        // for this same click (the toggle is outside the
                        // popup); don't immediately reopen it.
                        // The marker expires by itself: the release that
                        // clears it may have landed on an occluding surface
                        // and never reached the root handler.
                        let dismissed_recently = this
                            .combo_dismissed_at
                            .is_some_and(|since| since.elapsed() < COMBO_REVEAL);
                        let dismissed = this.combo_dismissed.take();
                        this.combo_dismissed_at = None;
                        if dismissed_recently && dismissed == Some(combo) {
                            cx.notify();
                            return;
                        }
                        this.combo_dismissed = None;
                        if this.open_combo == Some(combo) {
                            this.close_combo();
                        } else {
                            this.close_combo();
                            this.open_combo = Some(combo);
                            this.combo_opened_at = Some(Instant::now());
                            this.combo_closing = None;
                        }
                        cx.notify();
                    })),
            )
            .when(showing, |el| {
                el.child(
                    deferred(
                        div()
                            .id(ElementId::Name(format!("{id}-popup").into()))
                            .absolute()
                            .opacity(reveal)
                            .when(opens_upward, |el| {
                                el.bottom(px(CONTROL_HEIGHT + 4.0 - slide))
                            })
                            .when(!opens_upward, |el| el.top(px(CONTROL_HEIGHT + 4.0 - slide)))
                            .left_0()
                            .w_full()
                            .max_h(px(240.0))
                            .p(px(5.0))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .rounded(px(CONTROL_RADIUS + 1.0))
                            .bg(if dark {
                                theme.button_soft_fill
                            } else {
                                theme.input_surface
                            })
                            .border_1()
                            .border_color(lighting::rim(body_fill, dark))
                            .shadow(lighting::panel(dark))
                            .overflow_y_scroll()
                            // A list on its way out takes no clicks and lets
                            // them through to whatever is beneath it.
                            .when(is_open, |el| {
                                el.occlude().on_mouse_down_out(cx.listener(
                                    move |this, _event, _window, cx| {
                                        this.close_combo();
                                        // Remember which combo this mouse-down
                                        // dismissed so its own toggle's click
                                        // doesn't reopen it.
                                        this.combo_dismissed = Some(combo);
                                        this.combo_dismissed_at = Some(Instant::now());
                                        cx.notify();
                                    },
                                ))
                            })
                            .children(options_owned.into_iter().enumerate().map(
                                |(index, option)| {
                                    let on_select = on_select.clone();
                                    let highlighted = index == current_index;
                                    div()
                                        .id(ElementId::NamedInteger(
                                            format!("{id}-option").into(),
                                            index as u64,
                                        ))
                                        .h(px(28.0))
                                        .flex_none()
                                        .w_full()
                                        .px(px(9.0))
                                        .flex()
                                        .items_center()
                                        .rounded(px(CONTROL_RADIUS - 1.0))
                                        .text_size(px(12.5))
                                        .text_color(if highlighted {
                                            theme.button_label
                                        } else {
                                            theme.text_primary
                                        })
                                        .when(highlighted, |elem| {
                                            elem.bg(lighting::lit(theme.button_fill, 0.08))
                                        })
                                        .when(!highlighted, |elem| {
                                            elem.hover(move |style| style.bg(theme.row_odd))
                                        })
                                        .overflow_hidden()
                                        .child(SharedString::from(option))
                                        .when(is_open, |elem| {
                                            let on_select = on_select.clone();
                                            elem.cursor_pointer().on_click(cx.listener(
                                                move |this, _event, window, cx| {
                                                    this.close_combo();
                                                    on_select(this, index, window, cx);
                                                    cx.notify();
                                                },
                                            ))
                                        })
                                },
                            )),
                    )
                    .with_priority(100),
                )
            })
    }

    /// Hue slider. Bounds captured for drag math.
    pub fn render_hue_slider(&self, cx: &mut Context<RootView>) -> impl IntoElement {
        let theme = self.theme;
        let value = self.backend.theme_hue();
        let ratio = (value / 360.0).clamp(0.0, 1.0) as f32;
        let weak = cx.entity().downgrade();

        div()
            .id("hue-slider")
            .h(px(28.0))
            .flex_1()
            .px(px(6.0))
            .flex()
            .items_center()
            .relative()
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _window, cx| {
                    this.hue_dragging = true;
                    let position = event.position;
                    this.update_hue_from_mouse(position, cx);
                    cx.notify();
                }),
            )
            .child(
                // Track + fill + handle, positioned relative to the track.
                div()
                    .flex_1()
                    .h(px(5.0))
                    .rounded(px(2.5))
                    .bg(theme.input_border)
                    .relative()
                    .child({
                        // Capture the track bounds for drag hit-testing.
                        let weak = weak.clone();
                        canvas(
                            move |bounds, _window, cx| {
                                if let Some(root) = weak.upgrade() {
                                    root.update(cx, |root, _| {
                                        root.hue_slider_bounds = Some(bounds);
                                    });
                                }
                            },
                            |_bounds, _state, _window, _cx| {},
                        )
                        .absolute()
                        .size_full()
                    })
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(gpui::relative(ratio))
                            .rounded(px(2.5))
                            .bg(theme.accent)
                            .opacity(if theme.is_dark_mode { 0.45 } else { 0.55 }),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(-4.5))
                            .left(gpui::relative(ratio))
                            .ml(px(-7.0))
                            .w(px(14.0))
                            .h(px(14.0))
                            .rounded_full()
                            .bg(theme.input_surface)
                            .border_1()
                            .border_color(theme.input_border_accent),
                    ),
            )
    }
}
