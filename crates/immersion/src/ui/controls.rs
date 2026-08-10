//! Themed controls: ports of ActionButton.qml, PanelButton.qml,
//! ThemedSwitch.qml, ThemedSlider.qml, ThemedSpinBox.qml, ThemedComboBox.qml,
//! and the text-field/area chrome around TextInput.

use gpui::{
    Context, ElementId, Entity, MouseButton, SharedString, Window, canvas, deferred, div,
    prelude::*, px,
};

use crate::app::{ComboId, RootView};
use crate::text_input::TextInput;
use crate::theme::Theme;

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
    cx: &mut Context<RootView>,
    on_click: impl Fn(&mut RootView, &mut Window, &mut Context<RootView>) + 'static,
) -> impl IntoElement {
    let theme = *theme;
    let label: SharedString = text.to_string().into();
    div()
        .id(id.into())
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .bg(theme.button_fill)
        .hover(move |style| style.bg(theme.button_fill_hover))
        .active(move |style| style.bg(theme.button_fill_hover))
        .cursor_pointer()
        .text_size(px(13.0))
        .text_color(theme.button_label)
        .overflow_hidden()
        .child(label)
        .on_click(cx.listener(move |this, _event, window, cx| {
            on_click(this, window, cx);
        }))
}

/// PanelButton.qml: soft tinted button with default/primary/danger variants.
pub fn panel_button(
    id: impl Into<ElementId>,
    text: &str,
    variant: ButtonVariant,
    enabled: bool,
    theme: &Theme,
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
        .flex()
        .items_center()
        .justify_center()
        .h(px(38.0))
        .px(px(12.0))
        .rounded(px(8.0))
        .bg(fill)
        .text_size(px(13.0))
        .text_color(label_color)
        .overflow_hidden()
        .when(!theme.is_dark_mode, |el| {
            el.border_1().border_color(border_color)
        })
        .when(!enabled, |el| el.opacity(0.55))
        .when(enabled, move |el| {
            el.cursor_pointer()
                .hover(move |style| style.bg(fill_hover))
                .active(move |style| style.bg(fill_hover))
                .on_click(cx.listener(move |this, _event, window, cx| {
                    on_click(this, window, cx);
                }))
        })
        .child(label)
}

/// ThemedSwitch.qml.
pub fn themed_switch(
    id: impl Into<ElementId>,
    checked: bool,
    enabled: bool,
    theme: &Theme,
    cx: &mut Context<RootView>,
    on_toggle: impl Fn(&mut RootView, bool, &mut Window, &mut Context<RootView>) + 'static,
) -> impl IntoElement {
    let theme = *theme;
    let track_color = if checked { theme.accent } else { theme.input_border };
    let track_opacity = if !enabled {
        0.45
    } else if checked {
        if theme.is_dark_mode { 0.5 } else { 0.58 }
    } else {
        1.0
    };
    div()
        .id(id.into())
        .w(px(46.0))
        .h(px(26.0))
        .flex_none()
        .relative()
        .when(enabled, |el| {
            el.cursor_pointer()
                .on_click(cx.listener(move |this, _event, window, cx| {
                    on_toggle(this, !checked, window, cx);
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
                .top(px(3.0))
                .when(checked, |el| el.left(px(46.0 - 20.0 - 3.0)))
                .when(!checked, |el| el.left(px(3.0)))
                .w(px(20.0))
                .h(px(20.0))
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
