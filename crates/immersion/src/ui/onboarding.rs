//! OnboardingView.qml.

use gpui::{Context, FontWeight, div, prelude::*, px};

use crate::app::RootView;

impl RootView {
    pub fn render_onboarding(&self, cx: &mut Context<RootView>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .size_full()
            .bg(theme.app_background)
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(600.0))
                    .max_w_full()
                    .px(px(40.0))
                    .flex()
                    .flex_col()
                    .gap(px(32.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(16.0))
                            .child(
                                div()
                                    .text_size(px(48.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.text_primary)
                                    .text_center()
                                    .child("Welcome to Immersion"),
                            )
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .line_height(px(24.0))
                                    .text_color(theme.text_secondary)
                                    .text_center()
                                    .child("Open Settings to choose the folder where all your project files sit, and we'll take care of the rest."),
                            ),
                    )
                    .child(
                        div()
                            .id("onboarding-open-settings")
                            .h(px(48.0))
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.0))
                            .bg(theme.button_fill)
                            .hover(move |style| style.bg(theme.button_fill_hover))
                            .cursor_pointer()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.button_label)
                            .child("Open Settings")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.open_settings();
                                cx.notify();
                            })),
                    ),
            )
    }
}
