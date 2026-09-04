//! The app's side of the vampir toolkit.
//!
//! Every control lives in `vampir` and takes a [`vampir::Palette`]; the app
//! thinks in [`Theme`], which is a larger set of roles covering the graph,
//! the panels and the version nodes as well. These wrappers translate, so
//! the view code keeps passing the theme it already has.
//!
//! Anything with an `impl RootView` block below is app-specific: the combo
//! that knows which of this window's pop-ups opens upward, and the hue
//! slider that is wired to the backend's own setting.

use gpui::{Context, ElementId, Entity, ScrollHandle, Window, prelude::*};

use crate::app::RootView;
use crate::theme::Theme;

pub use vampir::controls::{ButtonVariant, CONTROL_HEIGHT, CONTROL_RADIUS, ComboDirection};
pub use vampir::easing::{ease_out_cubic, modal_opacity};
pub use vampir::scroll::ScrollAxis;
pub use vampir::text_input::TextInput;

/// Straight-line colour blend, under the name the view code uses.
pub use vampir::color::lerp as lerp_rgba;

pub fn caption(theme: &Theme, text: &str) -> impl IntoElement {
    vampir::controls::caption(theme.palette(), text)
}

pub fn panel_button(
    id: impl Into<ElementId>,
    text: &str,
    variant: ButtonVariant,
    enabled: bool,
    theme: &Theme,
    cx: &mut Context<RootView>,
    on_click: impl Fn(&mut RootView, &mut Window, &mut Context<RootView>) + 'static,
) -> impl IntoElement {
    vampir::controls::button(id, text, variant, enabled, theme.palette(), cx, on_click)
}

pub fn themed_switch(
    id: impl Into<ElementId>,
    checked: bool,
    enabled: bool,
    theme: &Theme,
    view: &RootView,
    cx: &mut Context<RootView>,
    on_toggle: impl Fn(&mut RootView, bool, &mut Window, &mut Context<RootView>) + 'static,
) -> impl IntoElement {
    vampir::controls::switch(id, checked, enabled, theme.palette(), view, cx, on_toggle)
}

#[allow(clippy::too_many_arguments)]
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
    vampir::controls::spinbox(
        id_prefix,
        value,
        min,
        max,
        enabled,
        edit_input,
        theme.palette(),
        cx,
        on_change,
    )
}

pub fn text_field(
    input: &Entity<TextInput>,
    theme: &Theme,
    window: &Window,
    cx: &Context<RootView>,
) -> impl IntoElement {
    vampir::controls::text_field(input, theme.palette(), window, cx)
}

pub fn text_area(
    input: &Entity<TextInput>,
    height: Option<f32>,
    enabled: bool,
    theme: &Theme,
    window: &Window,
    cx: &Context<RootView>,
) -> impl IntoElement {
    vampir::controls::text_area(input, height, enabled, theme.palette(), window, cx)
}

pub fn scrollbar(
    id: &'static str,
    handle: &ScrollHandle,
    axis: ScrollAxis,
    theme: &Theme,
    cx: &mut Context<RootView>,
) -> gpui::AnyElement {
    vampir::controls::scrollbar(id, handle, axis, theme.palette(), cx)
}

impl RootView {
    /// Pop-up button and its list.
    ///
    /// The main-file picker sits a few pixels above the bottom of the
    /// window, so its list opens upward; everything else has room below.
    pub fn render_combo(
        &self,
        id: &'static str,
        current_index: usize,
        options: &[String],
        width: Option<f32>,
        cx: &mut Context<RootView>,
        on_select: impl Fn(&mut RootView, usize, &mut Window, &mut Context<RootView>) + 'static,
    ) -> impl IntoElement {
        let direction = if id == "primary-file" {
            ComboDirection::Up
        } else {
            ComboDirection::Down
        };
        vampir::controls::combo(
            id,
            current_index,
            options,
            width,
            direction,
            self.theme.palette(),
            self,
            cx,
            on_select,
        )
    }

    /// Theme hue slider, wired to the backend's stored hue.
    pub fn render_hue_slider(&self, cx: &mut Context<RootView>) -> impl IntoElement {
        vampir::swatch::hue_slider(
            HUE_SLIDER,
            self.backend.theme_hue(),
            self.theme.palette(),
            cx,
        )
    }
}

/// The hue slider's track id, shared between the element and the drag
/// handler that turns a position back into degrees.
pub const HUE_SLIDER: &str = "hue-slider";
