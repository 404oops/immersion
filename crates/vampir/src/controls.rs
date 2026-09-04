//! The controls: buttons, a switch, a spin box, text field and area, pop-up
//! menus, sliders and overlay scrollbars.
//!
//! Every one is a free function generic over the host view, taking a
//! [`Palette`] for colour and a `Context<V>` for its callbacks, and
//! returning an element. Nothing is a struct with a builder, because
//! nothing here holds state between frames beyond what [`ControlState`]
//! already holds.

use std::rc::Rc;
use std::time::Instant;

use gpui::{
    Context, ElementId, Entity, FontWeight, MouseButton, MouseDownEvent, PathBuilder, ScrollHandle,
    SharedString, Window, canvas, deferred, div, point, prelude::*, px,
};

use crate::easing::{ease_in_cubic, ease_out_cubic, lerp_f32, progress};
use crate::lighting;
use crate::palette::Palette;
use crate::scroll::{SCROLLBAR_THICKNESS, ScrollAxis, ScrollDrag, THUMB_THICKNESS};
use crate::state::{COMBO_REVEAL, ComboId, ControlHost, SWITCH_SLIDE, TrackAxis};
use crate::text_input::TextInput;

/// Shared metrics: every field, pop-up and button is this tall and this
/// round, so a row of mixed controls lines up without per-control tuning.
pub const CONTROL_HEIGHT: f32 = 30.0;
pub const CONTROL_RADIUS: f32 = 6.0;

// ---- Captions ---------------------------------------------------------------

/// Section label: a quiet, medium-weight line that names a region without
/// competing with the content in it.
pub fn caption(palette: Palette, text: &str) -> impl IntoElement {
    div()
        .flex_none()
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.text_secondary)
        .whitespace_nowrap()
        .child(SharedString::from(text.to_string()))
}

// ---- Buttons ----------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ButtonVariant {
    /// Tinted secondary button. Most buttons are this.
    Soft,
    /// The single call to action in a group.
    Primary,
    /// Destructive.
    Danger,
}

impl ButtonVariant {
    /// (resting fill, hover fill, label).
    fn colors(self, palette: Palette) -> (gpui::Rgba, gpui::Rgba, gpui::Rgba) {
        match self {
            ButtonVariant::Primary => (
                palette.primary_fill,
                palette.primary_fill_hover,
                palette.primary_label,
            ),
            ButtonVariant::Danger => (
                palette.danger_fill,
                palette.danger_fill_hover,
                palette.danger_label,
            ),
            ButtonVariant::Soft => (
                palette.soft_fill,
                palette.soft_fill_hover,
                palette.soft_label,
            ),
        }
    }
}

/// Button lit from above: gradient fill, a highlight along the top edge, a
/// darker rim and a soft shadow. Fills its width, so put it in a sized slot.
pub fn button<V: ControlHost>(
    id: impl Into<ElementId>,
    text: &str,
    variant: ButtonVariant,
    enabled: bool,
    palette: Palette,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let dark = palette.is_dark;
    let (fill, fill_hover, label_color) = variant.colors(palette);
    let label: SharedString = text.to_string().into();

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
                // Pressed: no gradient at all, so the control reads as
                // pushed flat into the surface rather than merely darker.
                .active(move |style| style.bg(lighting::lit(lighting::shade(fill, -0.06), 0.0)))
                .on_click(cx.listener(move |this, _event, window, cx| {
                    on_click(this, window, cx);
                }))
        })
        .child(label)
}

// ---- Switch -----------------------------------------------------------------

/// Toggle switch.
///
/// The track colour and knob position cross over [`SWITCH_SLIDE`] with
/// [`ease_out_cubic`]. The instant of the toggle is recorded in
/// [`crate::ControlState`], so a first paint or a remount shows the end
/// state without animating: only a real toggle slides.
pub fn switch<V: ControlHost>(
    id: impl Into<ElementId>,
    checked: bool,
    enabled: bool,
    palette: Palette,
    view: &V,
    cx: &mut Context<V>,
    on_toggle: impl Fn(&mut V, bool, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    const KNOB: f32 = 20.0;
    const TRACK_W: f32 = 46.0;
    const PAD: f32 = 3.0;
    let left_off = PAD;
    let left_on = TRACK_W - KNOB - PAD;

    let id: ElementId = id.into();
    let track_off = palette.field_border;
    let track_on = palette.accent;
    let track_target = if checked { track_on } else { track_off };
    let track_source = if checked { track_off } else { track_on };
    let left_target = if checked { left_on } else { left_off };
    let left_source = if checked { left_off } else { left_on };
    // The on state is the accent at reduced strength: at full saturation a
    // switch shouts louder than the setting it controls.
    let track_opacity = if !enabled {
        0.45
    } else if checked {
        if palette.is_dark { 0.5 } else { 0.58 }
    } else {
        1.0
    };

    let t = view.control_state().anim_progress(&id, SWITCH_SLIDE);
    let eased = ease_out_cubic(t);
    let track_color = crate::color::lerp(track_source, track_target, eased);
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
                    this.control_state_mut().mark_changed(anim_key.clone());
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
                .bg(palette.field_surface)
                .border_1()
                .border_color(palette.field_border_strong),
        )
}

// ---- Spin box ---------------------------------------------------------------

/// Spin box: an editable value field between decrement and increment
/// buttons. The value is a real text input, so it can also be typed.
///
/// `edit_input` is the host's [`TextInput`] for the value; `value` is the
/// committed number the steppers work from.
#[allow(clippy::too_many_arguments)]
pub fn spinbox<V: ControlHost>(
    id_prefix: &'static str,
    value: i32,
    min: i32,
    max: i32,
    enabled: bool,
    edit_input: &Entity<TextInput>,
    palette: Palette,
    cx: &mut Context<V>,
    on_change: impl Fn(&mut V, i32, &mut Window, &mut Context<V>) + Clone + 'static,
) -> impl IntoElement {
    let dec = on_change.clone();
    let inc = on_change;

    type Step<V> = Box<dyn Fn(&mut V, &mut Window, &mut Context<V>) + 'static>;
    let step_button = |id: ElementId,
                       glyph: &'static str,
                       active: bool,
                       cx: &mut Context<V>,
                       handler: Step<V>| {
        div()
            .id(id)
            .w(px(22.0))
            .h(px(22.0))
            .rounded(px(3.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .text_color(palette.text_secondary)
            .when(!enabled, |el| el.opacity(0.45))
            // A stepper at its limit stays visible but stops responding:
            // removing it would shift the other one under the pointer.
            .when(enabled && active, move |el| {
                el.cursor_pointer()
                    .hover(move |style| style.bg(palette.soft_fill))
                    .active(move |style| style.bg(palette.soft_fill_hover))
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
        .bg(palette.field_surface)
        .border_1()
        .border_color(palette.field_border)
        .shadow(lighting::recessed(palette.is_dark))
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
            div()
                .flex_1()
                .px(px(4.0))
                .text_size(px(12.0))
                .text_color(palette.text_primary)
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

// ---- Text field and area ----------------------------------------------------

/// Recessed well, plus the accent glow while focused.
fn well_shadows(palette: Palette, focused: bool) -> Vec<gpui::BoxShadow> {
    let mut shadows = lighting::recessed(palette.is_dark);
    if focused {
        shadows.push(lighting::glow(palette.accent, 0.35, 3.0));
    }
    shadows
}

/// Single-line text field: a recessed well around a [`TextInput`].
///
/// The chrome takes the press itself and forwards it, so clicking the
/// padding either side of the text still places the caret instead of
/// missing the input entirely.
pub fn text_field<V: ControlHost>(
    input: &Entity<TextInput>,
    palette: Palette,
    window: &Window,
    cx: &Context<V>,
) -> impl IntoElement {
    let focused = input.read(cx).focus_handle.is_focused(window);
    let click_input = input.clone();
    div()
        .h(px(CONTROL_HEIGHT))
        .w_full()
        .px(px(9.0))
        .flex()
        .items_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(palette.field_surface)
        .border_1()
        .border_color(if focused {
            palette.accent
        } else {
            palette.field_border
        })
        .shadow(well_shadows(palette, focused))
        .overflow_hidden()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_host, event: &MouseDownEvent, window, cx| {
                click_input.update(cx, |input, cx| {
                    input.handle_chrome_click(event.position, window, cx)
                });
            }),
        )
        .child(input.clone())
}

/// Multi-line text area in the same recessed well.
///
/// With `height` it is fixed; without, it fills its flex slot down to a 40px
/// floor, so an area in a resizable panel grows with the panel instead of
/// clipping.
pub fn text_area<V: ControlHost>(
    input: &Entity<TextInput>,
    height: Option<f32>,
    enabled: bool,
    palette: Palette,
    window: &Window,
    cx: &Context<V>,
) -> impl IntoElement {
    let focused = input.read(cx).focus_handle.is_focused(window);
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
        .bg(palette.area_surface)
        .border_1()
        .border_color(if focused {
            palette.accent
        } else {
            palette.area_border
        })
        .shadow(well_shadows(palette, focused))
        .when(!enabled, |el| el.opacity(0.5))
        .overflow_y_scroll()
        .when(enabled, |el| {
            el.on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_host, event: &MouseDownEvent, window, cx| {
                    click_input.update(cx, |input, cx| {
                        input.handle_chrome_click(event.position, window, cx)
                    });
                }),
            )
        })
        .child(input.clone())
}

// ---- Pop-up menu ------------------------------------------------------------

/// Which way a pop-up's list opens.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ComboDirection {
    #[default]
    Down,
    /// For a pop-up near the bottom of a window, where a list opening
    /// downward would be cut off.
    Up,
}

/// Pop-up menu: a raised button showing the current option, with an accent
/// chevron chip, and a floating list when open.
///
/// `id` is both the element id and the pop-up's identity in
/// [`crate::ControlState`], so no two pop-ups in one view may share it.
#[allow(clippy::too_many_arguments)]
pub fn combo<V: ControlHost>(
    id: ComboId,
    current_index: usize,
    options: &[String],
    width: Option<f32>,
    direction: ComboDirection,
    palette: Palette,
    view: &V,
    cx: &mut Context<V>,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let dark = palette.is_dark;
    let state = view.control_state();
    let is_open = state.is_combo_open(id);
    // The list keeps rendering while it fades back out after a close.
    let closing_since = state.combo_closing.and_then(|(closing, since)| {
        (closing == id && since.elapsed() < COMBO_REVEAL).then_some(since)
    });
    let showing = is_open || closing_since.is_some();
    // Reveal progress: opacity, plus a short slide between the button and
    // the list's resting place. Opening eases out and closing eases in, so
    // both ends of the motion sit against the button.
    let reveal = if is_open {
        state
            .combo_opened_at
            .map(|since| ease_out_cubic(progress(since, COMBO_REVEAL)))
            .unwrap_or(1.0)
    } else if let Some(since) = closing_since {
        1.0 - ease_in_cubic(progress(since, COMBO_REVEAL))
    } else {
        1.0
    };
    let slide = 6.0 * (1.0 - reveal);
    let opens_upward = direction == ComboDirection::Up;

    let display: SharedString = options
        .get(current_index)
        .cloned()
        .unwrap_or_default()
        .into();
    let options_owned: Vec<String> = options.to_vec();
    let on_select = Rc::new(on_select);
    // In dark mode the button is a tinted fill; in light mode it is the same
    // white as a field, so a row of fields and pop-ups reads as one surface.
    let body_fill = if dark {
        palette.soft_fill
    } else {
        palette.field_surface
    };
    let chip_fill = palette.control_fill;
    let chevron_color: gpui::Hsla = palette.control_label.into();

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
                    palette.accent
                } else {
                    lighting::rim(body_fill, dark)
                })
                .shadow(lighting::raised(dark))
                .cursor_pointer()
                .text_size(px(12.5))
                .text_color(palette.text_primary)
                .whitespace_nowrap()
                .overflow_hidden()
                .hover(move |style| style.bg(lighting::lit(body_fill, 0.09)))
                .child(display)
                .child(chevron_chip(chip_fill, chevron_color, dark))
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    let state = this.control_state_mut();
                    // The list's own press-outside already closed this
                    // pop-up for this very click, the button being outside
                    // the list. Don't reopen it. The marker expires by
                    // itself, because the release that would clear it may
                    // land on an occluding surface and never arrive.
                    let dismissed_recently = state
                        .combo_dismissed_at
                        .is_some_and(|since| since.elapsed() < COMBO_REVEAL);
                    let dismissed = state.combo_dismissed.take();
                    state.combo_dismissed_at = None;
                    if dismissed_recently && dismissed == Some(id) {
                        cx.notify();
                        return;
                    }
                    if state.is_combo_open(id) {
                        state.close_combo();
                    } else {
                        state.open_combo(id);
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
                        .bg(body_fill)
                        .border_1()
                        .border_color(lighting::rim(body_fill, dark))
                        .shadow(lighting::panel(dark))
                        .overflow_y_scroll()
                        // A list on its way out takes no clicks and lets
                        // them through to whatever is beneath it.
                        .when(is_open, |el| {
                            el.occlude().on_mouse_down_out(cx.listener(
                                move |this, _event, _window, cx| {
                                    let state = this.control_state_mut();
                                    state.close_combo();
                                    state.combo_dismissed = Some(id);
                                    state.combo_dismissed_at = Some(Instant::now());
                                    cx.notify();
                                },
                            ))
                        })
                        .children(
                            options_owned
                                .into_iter()
                                .enumerate()
                                .map(|(index, option)| {
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
                                            palette.control_label
                                        } else {
                                            palette.text_primary
                                        })
                                        .when(highlighted, |elem| {
                                            elem.bg(lighting::lit(palette.control_fill, 0.08))
                                        })
                                        .when(!highlighted, |elem| {
                                            elem.hover(move |style| style.bg(palette.row_hover))
                                        })
                                        .overflow_hidden()
                                        .child(SharedString::from(option))
                                        .when(is_open, |elem| {
                                            let on_select = on_select.clone();
                                            elem.cursor_pointer().on_click(cx.listener(
                                                move |this, _event, window, cx| {
                                                    this.control_state_mut().close_combo();
                                                    on_select(this, index, window, cx);
                                                    cx.notify();
                                                },
                                            ))
                                        })
                                }),
                        ),
                )
                .with_priority(100),
            )
        })
}

/// The up/down chevrons on a pop-up button, drawn rather than set in a font
/// so they land on the same pixels whatever the UI face is.
fn chevron_chip(fill: gpui::Rgba, chevron: gpui::Hsla, dark: bool) -> impl IntoElement {
    div()
        .absolute()
        .right(px(5.0))
        .top(px((CONTROL_HEIGHT - 2.0 - 18.0) / 2.0))
        .w(px(18.0))
        .h(px(18.0))
        .rounded(px(CONTROL_RADIUS - 2.0))
        .bg(lighting::lit(fill, 0.12))
        .shadow(lighting::raised(dark))
        .child(
            canvas(
                |_bounds, _window, _cx| {},
                move |bounds, _state, window, _cx| {
                    let o = bounds.origin;
                    let mut builder = PathBuilder::stroke(px(1.5));
                    builder.move_to(point(o.x + px(5.5), o.y + px(7.5)));
                    builder.line_to(point(o.x + px(9.0), o.y + px(4.0)));
                    builder.line_to(point(o.x + px(12.5), o.y + px(7.5)));
                    builder.move_to(point(o.x + px(5.5), o.y + px(10.5)));
                    builder.line_to(point(o.x + px(9.0), o.y + px(14.0)));
                    builder.line_to(point(o.x + px(12.5), o.y + px(10.5)));
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, chevron);
                    }
                },
            )
            .size_full(),
        )
}

// ---- Sliders ----------------------------------------------------------------

/// How a slider's track behaves.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SliderTrack {
    /// Any value between the ends.
    #[default]
    Continuous,
    /// `stops` evenly spaced positions, marked with ticks, the first and
    /// last at the ends of the track. A value of 2 gives a two-position
    /// slider; anything below 2 falls back to continuous.
    Stepped { stops: u32 },
}

impl SliderTrack {
    /// The stop count, if this track has enough of them to snap to. The
    /// snapping itself happens in [`crate::ControlState::track_ratio_at`],
    /// so a drag stays quantised after the pointer has left the track.
    pub fn stops(self) -> Option<u32> {
        match self {
            SliderTrack::Stepped { stops } if stops >= 2 => Some(stops),
            _ => None,
        }
    }
}

/// Horizontal slider: a track, an accent fill up to `ratio` (0..=1) and a
/// round handle. With [`SliderTrack::Stepped`] it also draws a tick at each
/// stop and snaps to them.
///
/// The slider owns its drag. A press anywhere on the track jumps there and
/// starts one; every position, on press and while dragging, arrives at
/// [`ControlHost::track_dragged`] as the `x` of its point. The host only
/// has to pump the drag from wherever it tracks the pointer, with
/// [`crate::state::continue_drags`], and end it with
/// [`crate::state::end_drags`].
pub fn slider<V: ControlHost>(
    id: ComboId,
    ratio: f32,
    track: SliderTrack,
    palette: Palette,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let ratio = ratio.clamp(0.0, 1.0);
    let stops = track.stops();
    let weak = cx.entity().downgrade();

    div()
        .id(id)
        .h(px(28.0))
        .flex_1()
        .px(px(6.0))
        .flex()
        .items_center()
        .relative()
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                this.control_state_mut()
                    .begin_track_drag(id, TrackAxis::Horizontal, stops);
                if let Some((_, at)) = this.control_state().track_ratio_at(event.position) {
                    this.track_dragged(id, at, cx);
                }
                cx.notify();
            }),
        )
        .child(
            div()
                .flex_1()
                .h(px(5.0))
                .rounded(px(2.5))
                .bg(palette.field_border)
                .relative()
                .child(track_probe(id, weak))
                .children(stops.map(|stops| tick_marks(stops, palette)))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(gpui::relative(ratio))
                        .rounded(px(2.5))
                        .bg(palette.accent)
                        .opacity(if palette.is_dark { 0.45 } else { 0.55 }),
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
                        .bg(palette.field_surface)
                        .border_1()
                        .border_color(palette.field_border_strong),
                ),
        )
}

/// An invisible layer that records its own bounds into the control state as
/// it paints. Every draggable track needs this: a drag reads positions
/// against the track long after the pointer has left it.
pub(crate) fn track_probe<V: ControlHost>(
    id: ComboId,
    weak: gpui::WeakEntity<V>,
) -> impl IntoElement {
    canvas(
        move |bounds, _window, cx| {
            if let Some(host) = weak.upgrade() {
                host.update(cx, |host, _cx| {
                    host.control_state_mut().track_bounds.insert(id, bounds);
                });
            }
        },
        |_bounds, _state, _window, _cx| {},
    )
    .absolute()
    .size_full()
}

/// Ticks under a stepped track: one short mark per stop, the end ones
/// pulled inside the track so they are not clipped by its rounded caps.
fn tick_marks(stops: u32, palette: Palette) -> impl IntoElement {
    let last = (stops - 1) as f32;
    div()
        .absolute()
        .inset_0()
        .children((0..stops).map(move |stop| {
            let at = stop as f32 / last;
            div()
                .absolute()
                .top(px(7.0))
                .left(gpui::relative(at))
                .ml(px(-0.5))
                .w(px(1.0))
                .h(px(4.0))
                .bg(palette.field_border_strong)
                .opacity(0.7)
        }))
}

// ---- Checkbox ---------------------------------------------------------------

/// Checkbox with an optional label to its right. The whole row is the hit
/// target, label included, because a 14px square is a poor one.
pub fn checkbox<V: ControlHost>(
    id: impl Into<ElementId>,
    checked: bool,
    label: Option<&str>,
    enabled: bool,
    palette: Palette,
    cx: &mut Context<V>,
    on_toggle: impl Fn(&mut V, bool, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    const BOX: f32 = 15.0;
    let dark = palette.is_dark;
    let tick: gpui::Hsla = palette.control_label.into();
    let label: Option<SharedString> = label.map(|text| text.to_string().into());

    div()
        .id(id.into())
        .flex()
        .items_center()
        .gap(px(7.0))
        .flex_none()
        .when(!enabled, |el| el.opacity(0.5))
        .when(enabled, |el| {
            el.cursor_pointer()
                .on_click(cx.listener(move |this, _event, window, cx| {
                    on_toggle(this, !checked, window, cx);
                    cx.notify();
                }))
        })
        .child(
            div()
                .w(px(BOX))
                .h(px(BOX))
                .flex_none()
                .rounded(px(4.0))
                // Unchecked reads as an empty well, checked as a filled
                // control: the state is legible from the depth alone,
                // before the tick is even resolved.
                .when(checked, |el| {
                    el.bg(lighting::lit(palette.control_fill, 0.1))
                        .border_1()
                        .border_color(lighting::rim(palette.control_fill, dark))
                        .shadow(lighting::raised(dark))
                })
                .when(!checked, |el| {
                    el.bg(palette.field_surface)
                        .border_1()
                        .border_color(palette.field_border_strong)
                        .shadow(lighting::recessed(dark))
                })
                .when(checked, move |el| {
                    el.child(
                        canvas(
                            |_bounds, _window, _cx| {},
                            move |bounds, _state, window, _cx| {
                                let o = bounds.origin;
                                let mut builder = PathBuilder::stroke(px(1.75));
                                builder.move_to(point(o.x + px(3.5), o.y + px(7.5)));
                                builder.line_to(point(o.x + px(6.2), o.y + px(10.5)));
                                builder.line_to(point(o.x + px(11.5), o.y + px(4.5)));
                                if let Ok(path) = builder.build() {
                                    window.paint_path(path, tick);
                                }
                            },
                        )
                        .size_full(),
                    )
                }),
        )
        .children(label.map(|label| {
            div()
                .text_size(px(12.5))
                .text_color(palette.text_primary)
                .whitespace_nowrap()
                .child(label)
        }))
}

// ---- Segmented control ------------------------------------------------------

/// Segmented control: a recessed track with one raised segment for the
/// current choice. Use it instead of a pop-up when there are two or three
/// options and they are worth showing at once.
pub fn segmented<V: ControlHost>(
    id: &'static str,
    options: &[String],
    selected: usize,
    enabled: bool,
    palette: Palette,
    cx: &mut Context<V>,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let dark = palette.is_dark;
    let on_select = Rc::new(on_select);
    let fill = palette.control_fill;

    // Built up front rather than inside `.children(..)`: each segment needs
    // `cx` to make its listener, and a closure passed to `map` cannot hand
    // the same `&mut` out more than once.
    let mut segments: Vec<gpui::AnyElement> = Vec::with_capacity(options.len());
    for (index, option) in options.iter().enumerate() {
        // Fresh reborrow per segment: the listener closure takes `cx` by
        // move, and the loop needs it again next time round.
        let cx: &mut Context<V> = &mut *cx;
        let on_select = on_select.clone();
        let active = index == selected;
        segments.push(
            div()
                .id(ElementId::NamedInteger(
                    format!("{id}-segment").into(),
                    index as u64,
                ))
                .h_full()
                .flex_1()
                .px(px(10.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(CONTROL_RADIUS - 2.0))
                .text_size(px(12.0))
                .font_weight(if active {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(if active {
                    palette.control_label
                } else {
                    palette.text_secondary
                })
                .whitespace_nowrap()
                .overflow_hidden()
                .when(active, |el| {
                    el.bg(lighting::lit(fill, 0.09))
                        .shadow(lighting::raised(dark))
                })
                .when(enabled && !active, move |el| {
                    el.cursor_pointer()
                        .hover(move |style| style.bg(palette.row_hover))
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            on_select(this, index, window, cx);
                            cx.notify();
                        }))
                })
                .child(SharedString::from(option.clone()))
                .into_any_element(),
        );
    }

    div()
        .id(id)
        .h(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .gap(px(2.0))
        .p(px(2.0))
        .rounded(px(CONTROL_RADIUS))
        .bg(palette.field_surface)
        .border_1()
        .border_color(palette.field_border)
        .shadow(lighting::recessed(dark))
        .when(!enabled, |el| el.opacity(0.5))
        .children(segments)
}

// ---- Icon button ------------------------------------------------------------

/// Square button around whatever element the host draws: a glyph, an SVG, a
/// rendered image. `active` gives it the pressed-in look of a toggle that is
/// on.
#[allow(clippy::too_many_arguments)]
pub fn icon_button<V: ControlHost>(
    id: impl Into<ElementId>,
    icon: impl IntoElement,
    size: f32,
    active: bool,
    enabled: bool,
    palette: Palette,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let dark = palette.is_dark;
    let fill = if active {
        palette.control_fill
    } else {
        palette.soft_fill
    };

    div()
        .id(id.into())
        .w(px(size))
        .h(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(lighting::lit(fill, if active { 0.04 } else { 0.08 }))
        .border_1()
        .border_color(lighting::rim(fill, dark))
        .text_color(if active {
            palette.control_label
        } else {
            palette.text_secondary
        })
        .when(active, |el| el.shadow(lighting::recessed(dark)))
        .when(!active, |el| el.shadow(lighting::raised(dark)))
        .when(!enabled, |el| el.opacity(0.5))
        .when(enabled, move |el| {
            el.cursor_pointer()
                .hover(move |style| style.bg(lighting::lit(palette.soft_fill_hover, 0.1)))
                .on_click(cx.listener(move |this, _event, window, cx| {
                    on_click(this, window, cx);
                }))
        })
        .child(icon)
}

// ---- Progress and rules -----------------------------------------------------

/// Determinate progress bar: the same recessed track as a slider, filled to
/// `fraction` (0..=1) in the accent colour.
pub fn progress_bar(fraction: f32, palette: Palette) -> impl IntoElement {
    let fraction = fraction.clamp(0.0, 1.0);
    div()
        .h(px(6.0))
        .w_full()
        .flex_none()
        .rounded(px(3.0))
        .bg(palette.field_border)
        .shadow(lighting::recessed(palette.is_dark))
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(gpui::relative(fraction))
                .rounded(px(3.0))
                .bg(lighting::lit(palette.accent, 0.12)),
        )
}

/// Hairline rule. Horizontal by default; `vertical` for a column divider.
pub fn separator(vertical: bool, palette: Palette) -> impl IntoElement {
    let mut color: gpui::Hsla = palette.field_border.into();
    color.a = 0.9;
    div()
        .flex_none()
        .bg(color)
        .when(vertical, |el| el.w(px(1.0)).h_full())
        .when(!vertical, |el| el.h(px(1.0)).w_full())
}

// ---- Overlay scrollbar ------------------------------------------------------

/// Interactive overlay scrollbar: a draggable thumb, and a track click that
/// jumps. Put it inside a `.relative()` wrapper around the scroll container.
/// It renders nothing while the content fits.
///
/// The thumb is faint until hovered, because an overlay bar is a hint about
/// position rather than a control competing with the content.
pub fn scrollbar<V: ControlHost>(
    id: &'static str,
    handle: &ScrollHandle,
    axis: ScrollAxis,
    palette: Palette,
    cx: &mut Context<V>,
) -> gpui::AnyElement {
    use crate::scroll::{apply_scroll_drag, scrollbar_geometry};

    let Some(geometry) = scrollbar_geometry(handle, axis) else {
        return div().absolute().into_any_element();
    };
    let mut thumb_color: gpui::Hsla = palette.text_secondary.into();
    thumb_color.a = 0.24;

    let drag_handle = handle.clone();
    let track = div()
        .id(ElementId::Name(format!("scrollbar-{id}").into()))
        .absolute()
        // Block clicks and hover from bleeding into rows under the track,
        // but let wheel deltas pass so the strip is not a scroll dead zone.
        .block_mouse_except_scroll()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
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
                    // Track click: centre the thumb on the pointer.
                    geometry.thumb_len / 2.0
                };
                let drag = ScrollDrag {
                    handle: drag_handle.clone(),
                    axis,
                    grab,
                };
                apply_scroll_drag(&drag, position);
                this.control_state_mut().scroll_drag = Some(drag);
                cx.notify();
            }),
        )
        // Blocking the mouse also truncates hover at this strip: while the
        // pointer is inside it the host is not hovered, so the host's own
        // drag tracking sees neither the moves nor the release. Forward
        // both, the way an occluding modal surface has to.
        .on_mouse_move(
            cx.listener(|this, event: &gpui::MouseMoveEvent, window, cx| {
                this.forwarded_mouse_move(event, window, cx);
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|this, _event, window, cx| {
                this.forwarded_mouse_up(window, cx);
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

// ---- Chips ------------------------------------------------------------------

/// Shorter than [`CONTROL_HEIGHT`], because a wrapping field of thirty of
/// them at full control height reads as a wall rather than a set of choices.
pub const CHIP_HEIGHT: f32 = 26.0;

/// Which chips in a group are on.
#[derive(Clone, Copy, Debug)]
pub enum ChipSelection<'a> {
    /// One at a time, or none.
    One(Option<usize>),
    /// Any number, as a mask running parallel to the options. A mask shorter
    /// than the options leaves the rest off.
    Many(&'a [bool]),
}

impl ChipSelection<'_> {
    fn holds(&self, index: usize) -> bool {
        match self {
            ChipSelection::One(selected) => *selected == Some(index),
            ChipSelection::Many(mask) => mask.get(index).copied().unwrap_or(false),
        }
    }
}

/// A chip: a button sized to its own label rather than to its container.
///
/// Use these when a choice has more options than a [`segmented`] control can
/// carry but they are all worth showing at once — twenty lighting effects,
/// a set of tags, a filter bar. Lay them out in a wrapping row:
///
/// ```ignore
/// div().flex().flex_wrap().gap(px(6.0)).children(chips)
/// ```
///
/// [`chip_group`] does exactly that for a plain list of labels; reach for
/// `chip` directly when each one needs something of its own, such as a value
/// to send rather than an index.
pub fn chip<V: ControlHost>(
    id: impl Into<ElementId>,
    label: &str,
    selected: bool,
    enabled: bool,
    palette: Palette,
    cx: &mut Context<V>,
    on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let dark = palette.is_dark;
    // Selected takes the accent; the rest stay quiet, so one chip reads out
    // of a field of them at a glance.
    let (fill, fill_hover, label_color) = if selected {
        (
            palette.control_fill,
            palette.control_fill,
            palette.control_label,
        )
    } else {
        (
            palette.soft_fill,
            palette.soft_fill_hover,
            palette.soft_label,
        )
    };
    let text: SharedString = label.to_string().into();

    div()
        .id(id.into())
        .h(px(CHIP_HEIGHT))
        // The point of the whole control: no `w_full`, so the chip is as
        // wide as its label and its neighbours sit beside it.
        .flex_none()
        .px(px(11.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(lighting::lit(fill, 0.08))
        .border_1()
        .border_color(if selected {
            palette.accent
        } else {
            lighting::rim(fill, dark)
        })
        .shadow(lighting::raised(dark))
        .text_size(px(12.0))
        .font_weight(if selected {
            FontWeight::MEDIUM
        } else {
            FontWeight::NORMAL
        })
        .text_color(label_color)
        .whitespace_nowrap()
        .when(!enabled, |el| el.opacity(0.5))
        .when(enabled, move |el| {
            el.cursor_pointer()
                .hover(move |style| style.bg(lighting::lit(fill_hover, 0.11)))
                .active(move |style| style.bg(lighting::lit(lighting::shade(fill, -0.06), 0.0)))
                .on_click(cx.listener(move |this, _event, window, cx| {
                    on_click(this, window, cx);
                }))
        })
        .child(text)
}

/// A wrapping row of [`chip`]s, each sized to its label.
///
/// Single or multiple selection, depending on the [`ChipSelection`] handed
/// in; either way the callback reports the chip that was clicked and the
/// host decides what that means.
pub fn chip_group<V: ControlHost>(
    id: &'static str,
    options: &[String],
    selected: ChipSelection<'_>,
    enabled: bool,
    palette: Palette,
    cx: &mut Context<V>,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let on_select = Rc::new(on_select);

    let mut chips: Vec<gpui::AnyElement> = Vec::with_capacity(options.len());
    for (index, option) in options.iter().enumerate() {
        // Fresh reborrow per chip: the listener closure takes `cx` by move,
        // and the loop needs it again next time round.
        let cx: &mut Context<V> = &mut *cx;
        let on_select = on_select.clone();
        chips.push(
            chip(
                ElementId::NamedInteger(format!("{id}-chip").into(), index as u64),
                option,
                selected.holds(index),
                enabled,
                palette,
                cx,
                move |this, window, cx| on_select(this, index, window, cx),
            )
            .into_any_element(),
        );
    }

    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(6.0))
        .children(chips)
}

// ---- Radio group ------------------------------------------------------------

/// One choice in a [`radio_group`].
#[derive(Clone, Debug)]
pub struct Choice {
    pub label: SharedString,
    /// A second line under the label. Use it when the choice needs a reason,
    /// not to restate the label at greater length.
    pub detail: Option<SharedString>,
    pub enabled: bool,
}

impl Choice {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            detail: None,
            enabled: true,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Vertical list of mutually exclusive choices, each a dot and a label.
///
/// Reach for it over [`segmented`] when the choices need explaining, or when
/// there are more than about three: a segmented control sized for prose
/// stops looking like one control.
pub fn radio_group<V: ControlHost>(
    id: &'static str,
    choices: &[Choice],
    selected: usize,
    enabled: bool,
    palette: Palette,
    cx: &mut Context<V>,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let dark = palette.is_dark;
    let on_select = Rc::new(on_select);

    let mut rows: Vec<gpui::AnyElement> = Vec::with_capacity(choices.len());
    for (index, choice) in choices.iter().enumerate() {
        let cx: &mut Context<V> = &mut *cx;
        let on_select = on_select.clone();
        let active = index == selected;
        let live = enabled && choice.enabled;
        let detail = choice.detail.clone();
        rows.push(
            div()
                .id(ElementId::NamedInteger(
                    format!("{id}-choice").into(),
                    index as u64,
                ))
                .flex()
                .items_start()
                .gap(px(8.0))
                .py(px(4.0))
                .when(!live, |el| el.opacity(0.5))
                .when(live, move |el| {
                    el.cursor_pointer()
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            on_select(this, index, window, cx);
                            cx.notify();
                        }))
                })
                .child(
                    // The dot is a recessed well with a raised core, so the
                    // chosen one reads at a glance from its depth.
                    div()
                        .mt(px(2.0))
                        .w(px(15.0))
                        .h(px(15.0))
                        .flex_none()
                        .rounded_full()
                        .bg(palette.field_surface)
                        .border_1()
                        .border_color(if active {
                            palette.accent
                        } else {
                            palette.field_border_strong
                        })
                        .shadow(lighting::recessed(dark))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(active, |el| {
                            el.child(
                                div()
                                    .w(px(7.0))
                                    .h(px(7.0))
                                    .rounded_full()
                                    .bg(lighting::lit(palette.control_fill, 0.12)),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .child(
                            div()
                                .text_size(px(12.5))
                                .text_color(palette.text_primary)
                                .child(choice.label.clone()),
                        )
                        .children(detail.map(|detail| {
                            div()
                                .text_size(px(11.5))
                                .text_color(palette.text_secondary)
                                .child(detail)
                        })),
                )
                .into_any_element(),
        );
    }

    div().flex().flex_col().gap(px(2.0)).children(rows)
}

// ---- Search field -----------------------------------------------------------

/// Text field with a magnifier and, once there is something to clear, a
/// clear button.
///
/// The input is the host's, so filtering happens through its `on_change`
/// like any other field; this only adds the chrome that makes it read as a
/// search rather than as a name.
pub fn search_field<V: ControlHost>(
    id: &'static str,
    input: &Entity<TextInput>,
    palette: Palette,
    window: &Window,
    cx: &mut Context<V>,
) -> impl IntoElement {
    let focused = input.read(cx).focus_handle.is_focused(window);
    let has_text = !input.read(cx).content.is_empty();
    let glyph: gpui::Hsla = palette.text_secondary.into();
    let click_input = input.clone();
    let clear_input = input.clone();

    div()
        .h(px(CONTROL_HEIGHT))
        .w_full()
        .pl(px(28.0))
        .pr(px(if has_text { 26.0 } else { 9.0 }))
        .relative()
        .flex()
        .items_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(palette.field_surface)
        .border_1()
        .border_color(if focused {
            palette.accent
        } else {
            palette.field_border
        })
        .shadow(well_shadows(palette, focused))
        .overflow_hidden()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_host, event: &MouseDownEvent, window, cx| {
                click_input.update(cx, |input, cx| {
                    input.handle_chrome_click(event.position, window, cx)
                });
            }),
        )
        .child(
            div()
                .absolute()
                .left(px(8.0))
                .top(px((CONTROL_HEIGHT - 2.0 - 14.0) / 2.0))
                .w(px(14.0))
                .h(px(14.0))
                .child(
                    canvas(
                        |_bounds, _window, _cx| {},
                        move |bounds, _state, window, _cx| {
                            let o = bounds.origin;
                            let mut builder = PathBuilder::stroke(px(1.3));
                            // A ring approximated by a dodecagon: at 10px
                            // across, a circle and this are the same
                            // picture, and this needs no curve support.
                            let (cx_, cy, r) = (o.x + px(6.0), o.y + px(6.0), 4.3f32);
                            for step in 0..=12 {
                                let angle = step as f32 * std::f32::consts::TAU / 12.0;
                                let at = point(cx_ + px(r * angle.cos()), cy + px(r * angle.sin()));
                                if step == 0 {
                                    builder.move_to(at);
                                } else {
                                    builder.line_to(at);
                                }
                            }
                            builder.move_to(point(o.x + px(9.2), o.y + px(9.2)));
                            builder.line_to(point(o.x + px(13.0), o.y + px(13.0)));
                            if let Ok(path) = builder.build() {
                                window.paint_path(path, glyph);
                            }
                        },
                    )
                    .size_full(),
                ),
        )
        .child(input.clone())
        .when(has_text, |el| {
            el.child(
                div()
                    .id(ElementId::Name(format!("{id}-clear").into()))
                    .absolute()
                    .right(px(6.0))
                    .top(px((CONTROL_HEIGHT - 2.0 - 16.0) / 2.0))
                    .w(px(16.0))
                    .h(px(16.0))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .text_size(px(11.0))
                    .text_color(palette.text_secondary)
                    .hover(move |style| style.bg(palette.row_hover))
                    .on_click(cx.listener(move |_host, _event, _window, cx| {
                        clear_input.update(cx, |input, cx| input.set_text("", cx));
                        cx.notify();
                    }))
                    .child("\u{2715}"),
            )
        })
}

// ---- Spinner and badges -----------------------------------------------------

/// Indeterminate busy indicator: an arc that turns once a second.
///
/// It animates by being repainted, so a host must keep asking for frames
/// while one is on screen. That is deliberate: a spinner nobody can see
/// should not be holding the display link open.
pub fn spinner(size: f32, palette: Palette) -> impl IntoElement {
    let color: gpui::Hsla = palette.accent.into();
    // A wall-clock phase, so several spinners on one screen turn together
    // rather than each from its own start.
    let phase = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs_f32())
        .unwrap_or(0.0);
    let start = (phase % 1.0) * std::f32::consts::TAU;
    let radius = size / 2.0 - 1.5;

    div().w(px(size)).h(px(size)).flex_none().child(
        canvas(
            |_bounds, _window, _cx| {},
            move |bounds, _state, window, _cx| {
                let o = bounds.origin;
                let centre = point(o.x + px(size / 2.0), o.y + px(size / 2.0));
                let mut builder = PathBuilder::stroke(px(1.8));
                // Three quarters of a ring: the gap is what makes the
                // rotation visible at all.
                let steps = 18;
                for step in 0..=steps {
                    let angle = start + (step as f32 / steps as f32) * std::f32::consts::TAU * 0.75;
                    let at = point(
                        centre.x + px(radius * angle.cos()),
                        centre.y + px(radius * angle.sin()),
                    );
                    if step == 0 {
                        builder.move_to(at);
                    } else {
                        builder.line_to(at);
                    }
                }
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }
            },
        )
        .size_full(),
    )
}

/// What a [`badge`] is saying.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BadgeTone {
    /// A plain count or label.
    #[default]
    Neutral,
    /// Something the accent colour already means elsewhere in the view.
    Accent,
    /// Something wrong.
    Danger,
}

/// Small pill for a count, a state or a tag.
pub fn badge(text: &str, tone: BadgeTone, palette: Palette) -> impl IntoElement {
    let (fill, label) = match tone {
        BadgeTone::Neutral => (palette.soft_fill, palette.soft_label),
        BadgeTone::Accent => (palette.control_fill, palette.control_label),
        BadgeTone::Danger => (palette.danger_fill, palette.danger_label),
    };
    div()
        .flex_none()
        .h(px(17.0))
        .px(px(6.0))
        .flex()
        .items_center()
        .rounded(px(8.5))
        .bg(lighting::lit(fill, 0.07))
        .border_1()
        .border_color(lighting::rim(fill, palette.is_dark))
        .text_size(px(10.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(label)
        .whitespace_nowrap()
        .child(SharedString::from(text.to_string()))
}
