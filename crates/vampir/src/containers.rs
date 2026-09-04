//! Things that hold other things: a tab bar, a split divider, a collapsible
//! section and a modal dialog.
//!
//! These take content rather than data. Where a control renders a value, a
//! container renders whatever the host puts inside it, so each one takes
//! elements and returns a bigger element.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    AnyElement, Context, ElementId, FontWeight, MouseButton, MouseDownEvent, PathBuilder,
    SharedString, Window, canvas, deferred, div, point, prelude::*, px,
};

use crate::controls::{ButtonVariant, CONTROL_RADIUS, button};
use crate::easing::ease_out_cubic;
use crate::lighting;
use crate::palette::Palette;
use crate::state::{ComboId, ControlHost, SWITCH_SLIDE, TabDrag, TrackAxis};

// ---- Tab bar ----------------------------------------------------------------

/// One tab.
#[derive(Clone, Debug)]
pub struct Tab {
    pub label: SharedString,
    /// Drawn to the right of the label, for a count or a modified dot.
    pub badge: Option<SharedString>,
    pub closable: bool,
}

impl Tab {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            badge: None,
            closable: false,
        }
    }

    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn closable(mut self) -> Self {
        self.closable = true;
        self
    }
}

/// Horizontal tab bar with drag-to-reorder.
///
/// Pressing a tab selects it; dragging one past the midpoint of its
/// neighbour moves it, and the drop arrives at
/// [`ControlHost::tabs_reordered`] once the host ends the drag with
/// [`crate::state::end_drags`]. A press that never moves stays a plain
/// selection, so reordering costs nothing in ordinary use.
#[allow(clippy::too_many_arguments)]
pub fn tab_bar<V: ControlHost>(
    id: ComboId,
    tabs: &[Tab],
    selected: usize,
    palette: Palette,
    view: &V,
    cx: &mut Context<V>,
    on_select: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
    on_close: impl Fn(&mut V, usize, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let dark = palette.is_dark;
    let on_select = Rc::new(on_select);
    let on_close = Rc::new(on_close);
    let drag: Option<TabDrag> = view.control_state().tab_drag.clone();
    let dragging_here = drag.as_ref().filter(|drag| drag.bar == id).cloned();
    let weak = cx.entity().downgrade();

    // Reordered for display while a drag is in flight, so the tabs slide
    // past each other under the pointer instead of jumping on release.
    let order: Vec<usize> = match &dragging_here {
        Some(drag) if drag.moved => {
            let mut order: Vec<usize> = (0..tabs.len()).collect();
            if drag.from < order.len() && drag.to < order.len() {
                let moved = order.remove(drag.from);
                order.insert(drag.to, moved);
            }
            order
        }
        _ => (0..tabs.len()).collect(),
    };

    let mut rendered: Vec<AnyElement> = Vec::with_capacity(tabs.len());
    for (slot, &index) in order.iter().enumerate() {
        let cx: &mut Context<V> = &mut *cx;
        let Some(tab) = tabs.get(index) else { continue };
        let on_select = on_select.clone();
        let on_close = on_close.clone();
        let active = index == selected;
        let held = dragging_here
            .as_ref()
            .is_some_and(|drag| drag.moved && drag.to == slot);
        let weak = weak.clone();
        let fill = if active {
            palette.control_fill
        } else {
            palette.soft_fill
        };
        rendered.push(
            div()
                .id(ElementId::NamedInteger(
                    format!("{id}-tab").into(),
                    index as u64,
                ))
                .h(px(26.0))
                .flex_none()
                .px(px(10.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .rounded(px(CONTROL_RADIUS))
                .cursor_pointer()
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
                .when(active, |el| {
                    el.bg(lighting::lit(fill, 0.08))
                        .shadow(lighting::raised(dark))
                })
                .when(!active, |el| {
                    el.hover(move |style| style.bg(palette.row_hover))
                })
                // The tab under the hand lifts off the bar, which is the
                // whole feedback that a reorder is happening.
                .when(held, |el| el.opacity(0.75).shadow(lighting::panel(dark)))
                .child(
                    // Reports this tab's bounds so a drag can work out which
                    // slot the pointer is over.
                    canvas(
                        move |bounds, _window, cx| {
                            if let Some(host) = weak.upgrade() {
                                host.update(cx, |host, _cx| {
                                    let slots =
                                        host.control_state_mut().tab_bounds.entry(id).or_default();
                                    if slots.len() <= slot {
                                        slots.resize(slot + 1, bounds);
                                    }
                                    slots[slot] = bounds;
                                });
                            }
                        },
                        |_bounds, _state, _window, _cx| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _event: &MouseDownEvent, _window, cx| {
                        this.control_state_mut().tab_drag = Some(TabDrag {
                            bar: id,
                            from: slot,
                            to: slot,
                            moved: false,
                        });
                        cx.notify();
                    }),
                )
                // Selection on release, not on press: pressing is also how a
                // reorder starts, and a drag must not leave a trail of
                // selections behind it.
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, _event, window, cx| {
                        let was_reorder = this
                            .control_state()
                            .tab_drag
                            .as_ref()
                            .is_some_and(|drag| drag.moved);
                        if !was_reorder {
                            on_select(this, index, window, cx);
                        }
                        cx.notify();
                    }),
                )
                .child(tab.label.clone())
                .children(tab.badge.clone().map(|badge| {
                    div()
                        .text_size(px(10.5))
                        .text_color(palette.text_secondary)
                        .child(badge)
                }))
                .when(tab.closable, move |el| {
                    let on_close = on_close.clone();
                    el.child(
                        div()
                            .id(ElementId::NamedInteger(
                                format!("{id}-tab-close").into(),
                                index as u64,
                            ))
                            .w(px(14.0))
                            .h(px(14.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.0))
                            .text_size(px(10.0))
                            .hover(move |style| style.bg(palette.row_hover))
                            .on_click(cx.listener(move |this, _event, window, cx| {
                                on_close(this, index, window, cx);
                                cx.notify();
                            }))
                            .child("\u{2715}"),
                    )
                })
                .into_any_element(),
        );
    }

    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(2.0))
        .p(px(2.0))
        .overflow_x_scroll()
        .children(rendered)
}

// ---- Split divider ----------------------------------------------------------

/// Draggable divider between two panes.
///
/// It is only the handle: the host owns the fraction and sizes the panes
/// with it. Dragging reports a position along the whole split area, which
/// arrives at [`ControlHost::track_dragged`] as `x` for a vertical divider
/// and `y` for a horizontal one, clamped to `limits`.
///
/// `thickness` is the visible rule; the grab area is wider, because a 1px
/// hit target is a 1px hit target however good the rest of the UI is.
pub fn split_handle<V: ControlHost>(
    id: ComboId,
    vertical: bool,
    palette: Palette,
    view: &V,
    cx: &mut Context<V>,
) -> impl IntoElement {
    const GRAB: f32 = 9.0;
    let dragging = view.control_state().is_dragging(id);
    let axis = if vertical {
        TrackAxis::Horizontal
    } else {
        TrackAxis::Vertical
    };
    let mut rule: gpui::Hsla = palette.field_border.into();
    if dragging {
        rule = palette.accent.into();
    }

    div()
        .id(id)
        .flex_none()
        .when(vertical, |el| el.w(px(GRAB)).h_full().cursor_col_resize())
        .when(!vertical, |el| el.h(px(GRAB)).w_full().cursor_row_resize())
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                this.control_state_mut().begin_track_drag(id, axis, None);
                if let Some((_, at)) = this.control_state().track_ratio_at(event.position) {
                    this.track_dragged(id, at, cx);
                }
                cx.notify();
            }),
        )
        .child(
            div()
                .when(vertical, |el| el.w(px(1.0)).h_full())
                .when(!vertical, |el| el.h(px(1.0)).w_full())
                .bg(rule),
        )
}

/// The invisible probe a split's container needs, so the divider can turn a
/// pointer position into a fraction of the whole area. Put it inside the
/// element the two panes share, as an absolutely positioned child.
pub fn split_area<V: ControlHost>(id: ComboId, cx: &mut Context<V>) -> impl IntoElement {
    crate::controls::track_probe::<V>(id, cx.entity().downgrade())
}

// ---- Collapsible section ----------------------------------------------------

/// Disclosure section: a header that turns its chevron and reveals the body.
///
/// The host owns `expanded`, because whether a section is open usually
/// outlives the view and belongs with the rest of its settings. The turn of
/// the chevron is animated from [`ControlState`](crate::ControlState).
#[allow(clippy::too_many_arguments)]
pub fn collapsible<V: ControlHost>(
    id: &'static str,
    title: &str,
    expanded: bool,
    palette: Palette,
    view: &V,
    cx: &mut Context<V>,
    body: impl IntoElement,
    on_toggle: impl Fn(&mut V, bool, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let element_id: ElementId = ElementId::Name(format!("{id}-disclosure").into());
    let t = ease_out_cubic(
        view.control_state()
            .anim_progress(&element_id, SWITCH_SLIDE),
    );
    // The chevron turns from where it was, so a toggle mid-turn reverses
    // smoothly instead of snapping to the other end first.
    let turned = if expanded { t } else { 1.0 - t };
    let chevron: gpui::Hsla = palette.text_secondary.into();
    let anim_key = element_id.clone();

    div()
        .flex()
        .flex_col()
        .child(
            div()
                .id(ElementId::Name(format!("{id}-header").into()))
                .h(px(28.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .cursor_pointer()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.text_primary)
                .on_click(cx.listener(move |this, _event, window, cx| {
                    this.control_state_mut().mark_changed(anim_key.clone());
                    on_toggle(this, !expanded, window, cx);
                    cx.notify();
                }))
                .child(
                    div().w(px(12.0)).h(px(12.0)).flex_none().child(
                        canvas(
                            |_bounds, _window, _cx| {},
                            move |bounds, _state, window, _cx| {
                                // Drawn rather than rotated: gpui has no
                                // transform, so the chevron is rebuilt each
                                // frame at the angle it has reached.
                                let o = bounds.origin;
                                let angle = turned * std::f32::consts::FRAC_PI_2;
                                let (sin, cos) = angle.sin_cos();
                                let centre = (6.0f32, 6.0f32);
                                let rotate = |x: f32, y: f32| {
                                    point(
                                        o.x + px(centre.0 + x * cos - y * sin),
                                        o.y + px(centre.1 + x * sin + y * cos),
                                    )
                                };
                                let mut builder = PathBuilder::stroke(px(1.5));
                                builder.move_to(rotate(-1.8, -3.6));
                                builder.line_to(rotate(1.8, 0.0));
                                builder.line_to(rotate(-1.8, 3.6));
                                if let Ok(path) = builder.build() {
                                    window.paint_path(path, chevron);
                                }
                            },
                        )
                        .size_full(),
                    ),
                )
                .child(SharedString::from(title.to_string())),
        )
        .when(expanded, |el| {
            el.child(div().pl(px(18.0)).pb(px(4.0)).child(body))
        })
}

// ---- Modal dialog -----------------------------------------------------------

/// One button in a dialog's footer.
pub struct DialogButton<V> {
    pub id: ElementId,
    pub label: SharedString,
    pub variant: ButtonVariant,
    pub enabled: bool,
    #[allow(clippy::type_complexity)]
    pub on_click: Box<dyn Fn(&mut V, &mut Window, &mut Context<V>) + 'static>,
}

impl<V: ControlHost> DialogButton<V> {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        variant: ButtonVariant,
        on_click: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant,
            enabled: true,
            on_click: Box::new(on_click),
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Modal dialog: a scrim over the view and a centred panel with a title, a
/// body and a row of buttons.
///
/// `opacity` drives both the scrim and the panel, so a host animates the
/// whole thing with one number from
/// [`modal_opacity`](crate::easing::modal_opacity). At zero the dialog is
/// gone and this returns nothing, which is how a host stops rendering a
/// dialog that has finished leaving.
///
/// Buttons go last-is-rightmost, so the confirming one belongs at the end.
#[allow(clippy::too_many_arguments)]
pub fn dialog<V: ControlHost>(
    id: &'static str,
    title: &str,
    width: f32,
    opacity: f32,
    palette: Palette,
    cx: &mut Context<V>,
    body: impl IntoElement,
    buttons: Vec<DialogButton<V>>,
    on_dismiss: impl Fn(&mut V, &mut Window, &mut Context<V>) + 'static,
) -> Option<impl IntoElement> {
    if opacity <= 0.0 {
        return None;
    }
    let dark = palette.is_dark;
    let opacity = opacity.clamp(0.0, 1.0);

    let mut footer: Vec<AnyElement> = Vec::with_capacity(buttons.len());
    for spec in buttons {
        let cx: &mut Context<V> = &mut *cx;
        let on_click = spec.on_click;
        footer.push(
            div()
                .flex_none()
                .min_w(px(88.0))
                .child(button(
                    spec.id,
                    &spec.label,
                    spec.variant,
                    spec.enabled,
                    palette,
                    cx,
                    move |host, window, cx| on_click(host, window, cx),
                ))
                .into_any_element(),
        );
    }

    Some(
        deferred(
            div()
                .id(ElementId::Name(format!("{id}-scrim").into()))
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::Rgba {
                    a: palette.scrim().a * opacity,
                    ..palette.scrim()
                })
                .occlude()
                // A press on the scrim dismisses. A press inside the panel
                // must not, so the panel occludes in its own right.
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                        on_dismiss(this, window, cx);
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .id(ElementId::Name(format!("{id}-panel").into()))
                        .w(px(width))
                        .max_h(gpui::relative(0.86))
                        .opacity(opacity)
                        .flex()
                        .flex_col()
                        .gap(px(14.0))
                        .p(px(18.0))
                        .rounded(px(10.0))
                        .bg(if dark {
                            palette.soft_fill
                        } else {
                            palette.field_surface
                        })
                        .border_1()
                        .border_color(palette.field_border)
                        .shadow(lighting::faded(lighting::panel(dark), opacity))
                        .occlude()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(palette.text_primary)
                                .child(SharedString::from(title.to_string())),
                        )
                        .child(div().flex_1().overflow_hidden().child(body))
                        .child(div().flex().justify_end().gap(px(8.0)).children(footer)),
                ),
        )
        .with_priority(150),
    )
}

/// How long a dialog takes to arrive and to leave, re-exported so a host can
/// drive its own timers off the same numbers.
pub const DIALOG_ENTER: Duration = crate::easing::MODAL_ENTER;
pub const DIALOG_EXIT: Duration = crate::easing::MODAL_EXIT;
