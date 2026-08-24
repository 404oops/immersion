//! Themed controls: ports of ActionButton.qml, PanelButton.qml,
//! ThemedSwitch.qml, ThemedSlider.qml, ThemedSpinBox.qml, ThemedComboBox.qml,
//! and the text-field/area chrome around TextInput.

use std::time::{Duration, Instant};

use gpui::{
    Context, ElementId, Entity, MouseButton, Rgba, SharedString, Window, canvas, deferred, div,
    prelude::*, px,
};

use crate::app::{ComboId, RootView};
use crate::text_input::TextInput;
use crate::theme::Theme;

/// QML Easing.OutCubic.
pub fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// QML Easing.InCubic (ThemedPopup exit, etc.).
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

/// ActionButton.qml: loud accent button (project rows, settings header).
pub fn action_button(
    id: impl Into<ElementId>,
    text: &str,
    theme: &Theme,
    _view: &RootView,
    cx: &mut Context<RootView>,
    on_click: impl Fn(&mut RootView, &mut Window, &mut Context<RootView>) + 'static,
) -> impl IntoElement {
    let theme = *theme;
    let label: SharedString = text.to_string().into();
    div()
        .id(id.into())
        .size_full()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _event, window, cx| {
            on_click(this, window, cx);
        }))
        .child(
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.0))
                .bg(theme.button_fill)
                .hover(move |style| style.bg(theme.button_fill_hover))
                .text_size(px(13.0))
                .text_color(theme.button_label)
                .overflow_hidden()
                .child(label),
        )
}

/// PanelButton.qml: soft tinted button with default/primary/danger variants.
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
    let (fill, fill_hover, label_color, border_color) = match variant {
        ButtonVariant::Primary => (
            theme.button_primary_fill,
            theme.button_primary_fill_hover,
            theme.button_primary_label,
            theme.button_primary_border,
        ),
        ButtonVariant::Danger => (
            theme.button_danger_fill,
            theme.button_danger_fill_hover,
            theme.button_danger_label,
            theme.button_danger_border,
        ),
        ButtonVariant::Soft => (
            theme.button_soft_fill,
            theme.button_soft_fill_hover,
            theme.button_soft_label,
            theme.button_soft_border,
        ),
    };
    let label: SharedString = text.to_string().into();
    let on_click: ClickHandler = Box::new(on_click);

    div()
        .id(id.into())
        .h(px(38.0))
        .flex()
        .when(!enabled, |el| el.opacity(0.55))
        .when(enabled, move |el| {
            el.cursor_pointer().on_click(cx.listener(move |this, _event, window, cx| {
                on_click(this, window, cx);
            }))
        })
        .child(
            div()
                .h_full()
                .px(px(12.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.0))
                .bg(fill)
                .when(enabled, |el| el.hover(move |style| style.bg(fill_hover)))
                .text_size(px(13.0))
                .text_color(label_color)
                .overflow_hidden()
                .when(!theme.is_dark_mode, |el| {
                    el.border_1().border_color(border_color)
                })
                .child(label),
        )
}

/// ThemedSwitch.qml.
///
/// QML Behaviors: track `color` and thumb `x` animate over 140ms with
/// `Easing.OutCubic`. `switch_anim` records the toggle instant so the first
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
                    this.switch_anim
                        .insert(anim_key.clone(), Instant::now());
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

/// ThemedSpinBox.qml (value display + increment/decrement buttons).
pub fn themed_spinbox(
    id_prefix: &'static str,
    value: i32,
    min: i32,
    max: i32,
    enabled: bool,
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
            .w(px(24.0))
            .h(px(24.0))
            .rounded(px(4.0))
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
        .h(px(32.0))
        .flex_none()
        .rounded(px(6.0))
        .bg(theme.input_fill)
        .border_1()
        .border_color(theme.input_border_accent)
        .when(!enabled, |el| el.opacity(0.55))
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
                .text_size(px(12.0))
                .text_color(theme.text_primary)
                .text_center()
                .child(value.to_string()),
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

/// ThemedTextField.qml chrome around a TextInput entity.
pub fn text_field(
    input: &Entity<TextInput>,
    theme: &Theme,
    window: &Window,
    cx: &Context<RootView>,
) -> impl IntoElement {
    let focused = input.read(cx).focus_handle.is_focused(window);
    let theme = *theme;
    div()
        .h(px(30.0))
        .w_full()
        .px(px(10.0))
        .flex()
        .items_center()
        .rounded(px(5.0))
        .bg(theme.input_fill)
        .when(focused, |el| {
            el.border_2().border_color(theme.accent)
        })
        .when(!focused, |el| {
            el.border_1().border_color(theme.input_border_accent)
        })
        .overflow_hidden()
        .child(input.clone())
}

/// ThemedTextArea.qml chrome around a multi-line TextInput entity.
pub fn text_area(
    input: &Entity<TextInput>,
    height: f32,
    enabled: bool,
    theme: &Theme,
    window: &Window,
    cx: &Context<RootView>,
) -> impl IntoElement {
    let focused = input.read(cx).focus_handle.is_focused(window);
    let theme = *theme;
    div()
        .id(ElementId::Name(
            format!("text-area-{}", input.entity_id()).into(),
        ))
        .h(px(height))
        .w_full()
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(6.0))
        .bg(theme.vm_input_surface)
        .when(focused, |el| {
            el.border_2().border_color(theme.input_border_accent)
        })
        .when(!focused, |el| el.border_1().border_color(theme.vm_border))
        .when(!enabled, |el| el.opacity(0.55))
        .overflow_y_scroll()
        .child(input.clone())
}

impl RootView {
    /// ThemedComboBox.qml. Renders the closed control and, when open, a
    /// deferred dropdown list anchored below it.
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
        let is_open = self.open_combo == Some(combo);
        let display: SharedString = options
            .get(current_index)
            .cloned()
            .unwrap_or_default()
            .into();
        let options_owned: Vec<String> = options.to_vec();
        let on_select = std::rc::Rc::new(on_select);

        div()
            .relative()
            .when_some(width, |el, w| el.w(px(w)).flex_none())
            .when(width.is_none(), |el| el.w_full())
            .child(
                div()
                    .id(ElementId::Name(format!("{id}-toggle").into()))
                    .h(px(30.0))
                    .w_full()
                    .pl(px(10.0))
                    .pr(px(24.0))
                    .flex()
                    .items_center()
                    .rounded(px(5.0))
                    .bg(theme.input_fill)
                    .when(is_open, |el| el.border_2().border_color(theme.accent))
                    .when(!is_open, |el| {
                        el.border_1().border_color(theme.input_border_accent)
                    })
                    .cursor_pointer()
                    .text_size(px(12.0))
                    .text_color(theme.text_primary)
                    .overflow_hidden()
                    .child(display)
                    .child(
                        div()
                            .absolute()
                            .right(px(10.0))
                            .text_size(px(11.0))
                            .text_color(theme.accent)
                            .child("\u{25be}"),
                    )
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.open_combo = if this.open_combo == Some(combo) {
                            None
                        } else {
                            Some(combo)
                        };
                        cx.notify();
                    })),
            )
            .when(is_open, |el| {
                el.child(deferred(
                    div()
                        .id(ElementId::Name(format!("{id}-popup").into()))
                        .absolute()
                        .top(px(32.0))
                        .left_0()
                        .w_full()
                        .max_h(px(220.0))
                        .p(px(3.0))
                        .rounded(px(5.0))
                        .bg(theme.input_fill)
                        .border_1()
                        .border_color(theme.input_border_accent)
                        .overflow_y_scroll()
                        .occlude()
                        .on_mouse_down_out(cx.listener(|this, _event, _window, cx| {
                            this.open_combo = None;
                            cx.notify();
                        }))
                        .children(options_owned.into_iter().enumerate().map(|(index, option)| {
                            let on_select = on_select.clone();
                            let highlighted = index == current_index;
                            div()
                                .id(ElementId::NamedInteger(
                                    format!("{id}-option").into(),
                                    index as u64,
                                ))
                                .h(px(28.0))
                                .w_full()
                                .pl(px(8.0))
                                .flex()
                                .items_center()
                                .rounded(px(4.0))
                                .text_size(px(12.0))
                                .text_color(theme.text_primary)
                                .when(highlighted, |elem| elem.bg(theme.selection))
                                .when(!highlighted, |elem| {
                                    elem.hover(move |style| style.bg(theme.row_even))
                                })
                                .cursor_pointer()
                                .overflow_hidden()
                                .child(SharedString::from(option))
                                .on_click(cx.listener(move |this, _event, window, cx| {
                                    this.open_combo = None;
                                    on_select(this, index, window, cx);
                                    cx.notify();
                                }))
                        })),
                )
                .with_priority(100))
            })
    }

    /// ThemedSlider.qml (hue slider). Bounds captured for drag math.
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
