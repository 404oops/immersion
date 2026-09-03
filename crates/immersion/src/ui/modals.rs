//! Modal layer: popup sizing/scrims, the settings window, the projects-folder
//! layout dialog, and the confirm dialog.

use gpui::{
    AnyElement, Context, ElementId, FontWeight, PathPromptOptions, SharedString, Window, div,
    prelude::*, px,
};

use crate::app::{ComboId, ConfirmAction, ConfirmState, RootView};
use crate::theme::MODAL_EDGE_PADDING;
use crate::theme::MODAL_PANEL_RADIUS;
use crate::ui::controls::{
    ButtonVariant, ScrollAxis, modal_opacity, panel_button, scrollbar, themed_spinbox,
    themed_switch,
};

/// How tall a popup panel is: a fixed frame, or one that hugs its content up
/// to a cap (short dialogs shouldn't paint a half-empty panel).
#[derive(Clone, Copy)]
enum PopupHeight {
    Fixed(f32),
    FitContent(f32),
}

/// ThemedPopup width/height rule.
fn popup_size(window: &Window, min: (f32, f32), max: (f32, f32), pref: (f32, f32)) -> (f32, f32) {
    let viewport = window.viewport_size();
    let available_w = (f32::from(viewport.width) - MODAL_EDGE_PADDING).max(0.0);
    let available_h = (f32::from(viewport.height) - MODAL_EDGE_PADDING).max(0.0);
    (
        available_w.min(min.0.max(max.0.min(pref.0))),
        available_h.min(min.1.max(max.1.min(pref.1))),
    )
}

impl RootView {
    pub fn render_modal_layer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> Vec<AnyElement> {
        let theme = self.theme;
        let mut layers: Vec<AnyElement> = Vec::new();

        if !self.any_modal_open() && self.confirm.is_none() {
            return layers;
        }

        // Topmost visible popup drives scrim opacity.
        let (scrim_opacity, _) = if self.confirm.is_some() {
            modal_opacity(self.confirm_enter_at, self.confirm_exit_at)
        } else if self.layout_dialog.open {
            modal_opacity(self.layout_enter_at, self.layout_exit_at)
        } else if self.settings_open {
            modal_opacity(self.settings_enter_at, self.settings_exit_at)
        } else {
            (1.0, false)
        };

        // Full-window scrim behind modal popups. It occludes the root, so it
        // must forward the shared drag tracking (hue slider, scrollbars).
        layers.push(
            div()
                .id("modal-scrim")
                .absolute()
                .inset_0()
                .bg(theme.modal_scrim)
                .opacity(scrim_opacity)
                .occlude()
                .on_mouse_move(
                    cx.listener(|this, event: &gpui::MouseMoveEvent, _window, cx| {
                        this.global_mouse_move(event, cx);
                    }),
                )
                .on_mouse_up(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _event, _window, cx| {
                        this.global_mouse_up(cx);
                    }),
                )
                .into_any_element(),
        );

        if self.settings_open {
            layers.push(self.render_settings_modal(window, cx).into_any_element());
        }

        // Nested scrim for child dialogs (layout dialog over settings, confirm).
        if self.child_dialog_open() {
            let (nested_opacity, _) = if self.confirm.is_some() {
                modal_opacity(self.confirm_enter_at, self.confirm_exit_at)
            } else {
                modal_opacity(self.layout_enter_at, self.layout_exit_at)
            };
            layers.push(
                div()
                    .id("nested-modal-scrim")
                    .absolute()
                    .inset_0()
                    .bg(theme.modal_scrim)
                    .opacity(0.72 * nested_opacity)
                    .occlude()
                    .into_any_element(),
            );
        }

        if self.layout_dialog.open {
            layers.push(self.render_layout_dialog(window, cx).into_any_element());
        }
        if self.confirm.is_some() {
            layers.push(self.render_confirm_dialog(window, cx).into_any_element());
        }

        layers
    }

    /// Centers a ThemedPopup; opacity fade on enter/exit applied once to the
    /// whole panel (chrome + content).
    fn popup_frame(
        &self,
        id: &'static str,
        size: (f32, PopupHeight),
        content: AnyElement,
        enter_at: Option<std::time::Instant>,
        exit_at: Option<std::time::Instant>,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let (opacity, _) = modal_opacity(enter_at, exit_at);
        div()
            .id(ElementId::Name(format!("{id}-host").into()))
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id(ElementId::Name(id.to_string().into()))
                    .w(px(size.0))
                    .map(|el| match size.1 {
                        PopupHeight::Fixed(h) => el.h(px(h)),
                        PopupHeight::FitContent(max) => el.max_h(px(max)),
                    })
                    .rounded(px(MODAL_PANEL_RADIUS))
                    .bg(theme.vm_panel)
                    .border_1()
                    .border_color(theme.modal_border)
                    .occlude()
                    // The occluding panel swallows the root's mouse stream;
                    // forward the shared drag tracking (hue slider drags
                    // happen INSIDE this panel).
                    .on_mouse_move(cx.listener(
                        |this, event: &gpui::MouseMoveEvent, _window, cx| {
                            this.global_mouse_move(event, cx);
                        },
                    ))
                    .on_mouse_up(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _event, _window, cx| {
                            this.global_mouse_up(cx);
                        }),
                    )
                    .overflow_hidden()
                    .opacity(opacity)
                    .child(content),
            )
    }

    // ---- Settings window ---------------------------------------------------

    fn render_settings_modal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let size = popup_size(window, (560.0, 480.0), (760.0, 720.0), (680.0, 640.0));

        let divider =
            |theme: &crate::theme::Theme| div().h(px(1.0)).w_full().flex_none().bg(theme.vm_border);

        let has_folder = self.backend.has_projects_folder();
        let folder_path = self.backend.projects_folder_path().to_string();
        let layout_name = self.backend.projects_folder_layout().to_string();
        let launch_supported = self.backend.launch_at_startup_supported();
        let launch_at_startup = self.backend.launch_at_startup();
        let sort_options = vec!["Name".to_string(), "Last Opened".to_string()];
        let sort_index = if self.backend.sort_mode() == "Last Opened" {
            1
        } else {
            0
        };
        let log_options = vec!["Info".to_string(), "Debug".to_string()];
        let log_index = if self.backend.log_level() == "Debug" {
            1
        } else {
            0
        };
        let scheme_options = vec![
            "System".to_string(),
            "Light".to_string(),
            "Dark".to_string(),
        ];
        let scheme_index = match self.backend.color_scheme_mode_string() {
            "Light" => 1,
            "Dark" => 2,
            _ => 0,
        };
        let dev_details = self.show_dev_details();
        let retention = self.backend.snapshot_retention();
        let retention_edit = self.retention_input.clone();
        let notifications = self.backend.notifications_enabled();
        let hue_value = self.backend.theme_hue();

        let section_label = move |text: &str| {
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .text_color(theme.vm_text_primary)
                .child(SharedString::from(text.to_string()))
        };
        let meta_label = move |text: &str| {
            div()
                .text_size(px(12.0))
                .line_height(px(16.0))
                .text_color(theme.vm_text_meta)
                .child(SharedString::from(text.to_string()))
        };

        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(12.0))
            .gap(px(10.0))
            // Header.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .text_size(px(19.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(theme.vm_text_primary)
                            .child("Settings"),
                    )
                    .child(div().flex_1())
                    .child(div().w(px(88.0)).child(panel_button(
                        "settings-close",
                        "Close",
                        ButtonVariant::Soft,
                        true,
                        &theme,
                        self,
                        cx,
                        |this, _w, cx| {
                            this.request_close_settings(cx);
                        },
                    ))),
            )
            // Scrollable body.
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .rounded(px(8.0))
                    .bg(theme.vm_side_panel)
                    .border_1()
                    .border_color(theme.vm_border)
                    .overflow_hidden()
                    .relative()
                    .child(
                        div()
                            .id("settings-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.settings_scroll)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(14.0))
                                    .px(px(14.0))
                                    .py(px(12.0))
                                    // Projects folder.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.0))
                                            .child(section_label("Projects folder"))
                                            .child(meta_label(if folder_path.is_empty() {
                                                "No folder selected yet"
                                            } else {
                                                &folder_path
                                            }))
                                            .child(panel_button(
                                                "choose-folder",
                                                if has_folder {
                                                    "Add Projects Folder\u{2026}"
                                                } else {
                                                    "Choose Projects Folder\u{2026}"
                                                },
                                                ButtonVariant::Primary,
                                                true,
                                                &theme,
                                                self,
                                                cx,
                                                |this, _window, cx| {
                                                    this.prompt_for_projects_folder(cx);
                                                },
                                            )),
                                    )
                                    // Folder layout.
                                    .when(has_folder, |el| {
                                        el.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap(px(8.0))
                                                .child(section_label("Folder layout"))
                                                .when(dev_details, |el| {
                                                    el.child(meta_label(
                                                        "Stored in this folder as .immersion/settings.json",
                                                    ))
                                                })
                                                .child(
                                                    div()
                                                        .text_size(px(12.0))
                                                        .text_color(theme.vm_text_primary)
                                                        .child(SharedString::from(format!(
                                                            "Current: {layout_name}"
                                                        ))),
                                                )
                                                .child(panel_button(
                                                    "change-layout",
                                                    "Change Folder Layout",
                                                    ButtonVariant::Soft,
                                                    !folder_path.is_empty(),
                                                    &theme,
                                                    self,
                                                    cx,
                                                    |this, window, cx| {
                                                        this.open_layout_dialog_for_current_folder(window, cx);
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .child(divider(&theme))
                                    })
                                    // Launch at login.
                                    .when(launch_supported, |el| {
                                        el.child(
                                            div()
                                                .flex()
                                                .items_center()
                                                .gap(px(12.0))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .flex()
                                                        .flex_col()
                                                        .gap(px(4.0))
                                                        .child(section_label("Launch at login"))
                                                        .child(meta_label(
                                                            "Start Immersion automatically when you sign in.",
                                                        )),
                                                )
                                                .child(themed_switch(
                                                    "launch-switch",
                                                    launch_at_startup,
                                                    true,
                                                    &theme,
                                                    self,
                                                    cx,
                                                    |this, checked, _w, cx| {
                                                        this.backend.set_launch_at_startup(checked);
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                        .child(divider(&theme))
                                    })
                                    // Default sort mode.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.0))
                                            .child(section_label("Default sort mode"))
                                            .child(self.render_combo(
                                                ComboId::SortSettings,
                                                "sort-settings",
                                                sort_index,
                                                &sort_options,
                                                None,
                                                cx,
                                                |this, index, _w, _cx| {
                                                    let mode = if index == 1 {
                                                        "Last Opened"
                                                    } else {
                                                        "Name"
                                                    };
                                                    this.backend.set_sort_mode(mode);
                                                },
                                            )),
                                    )
                                    .child(divider(&theme))
                                    // Activity log level.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.0))
                                            .child(section_label("Activity log level"))
                                            .child(meta_label(
                                                "Debug adds scan and watcher detail to the log, and shows where things are stored on disk.",
                                            ))
                                            .child(self.render_combo(
                                                ComboId::LogLevel,
                                                "log-level",
                                                log_index,
                                                &log_options,
                                                None,
                                                cx,
                                                |this, index, _w, _cx| {
                                                    let level =
                                                        if index == 1 { "Debug" } else { "Info" };
                                                    this.backend.set_log_level(level);
                                                },
                                            ))
                                            // The activity log itself lives here now
                                            // (the main view's bottom panel shows
                                            // project details instead).
                                            .child(
                                                div()
                                                    .h(px(180.0))
                                                    .w_full()
                                                    .rounded(px(6.0))
                                                    .bg(theme.vm_input_surface)
                                                    .border_1()
                                                    .border_color(theme.vm_border)
                                                    .p(px(8.0))
                                                    .child(self.render_activity_log(cx)),
                                            ),
                                    )
                                    .child(divider(&theme))
                                    // Snapshot retention.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.0))
                                            .child(section_label("Snapshot retention"))
                                            .child(meta_label(
                                                "Number of recent versions that keep fast uncompressed copies per file.",
                                            ))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(12.0))
                                                    .child(themed_spinbox(
                                                        "retention",
                                                        retention,
                                                        1,
                                                        50,
                                                        true,
                                                        &retention_edit,
                                                        &theme,
                                                        cx,
                                                        |this, value, _w, cx| {
                                                            this.backend
                                                                .set_snapshot_retention(value);
                                                            cx.notify();
                                                        },
                                                    ))
                                                    .child(meta_label("recent versions")),
                                            ),
                                    )
                                    .child(divider(&theme))
                                    // Save notifications.
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(12.0))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(4.0))
                                                    .child(section_label("Save notifications"))
                                                    .child(meta_label(
                                                        "Show a notification when a new version is saved.",
                                                    )),
                                            )
                                            .child(themed_switch(
                                                "notifications-switch",
                                                notifications,
                                                true,
                                                &theme,
                                                self,
                                                cx,
                                                |this, checked, _w, cx| {
                                                    this.backend
                                                        .set_notifications_enabled(checked);
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    .child(divider(&theme))
                                    // Appearance.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.0))
                                            .child(section_label("Appearance"))
                                            .child(self.render_combo(
                                                ComboId::ColorScheme,
                                                "color-scheme",
                                                scheme_index,
                                                &scheme_options,
                                                None,
                                                cx,
                                                |this, index, _w, cx| {
                                                    let mode = match index {
                                                        1 => "Light",
                                                        2 => "Dark",
                                                        _ => "System",
                                                    };
                                                    this.backend.set_color_scheme_mode(mode);
                                                    this.color_scheme =
                                                        this.backend.color_scheme_mode();
                                                    this.recompute_theme(cx);
                                                },
                                            )),
                                    )
                                    .child(divider(&theme))
                                    // Theme hue.
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.0))
                                            .child(section_label("Theme hue"))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(12.0))
                                                    .child(self.render_hue_slider(cx))
                                                    .child(
                                                        div()
                                                            .w(px(42.0))
                                                            .flex_none()
                                                            .text_size(px(12.0))
                                                            .text_color(theme.vm_text_meta)
                                                            .text_right()
                                                            .child(SharedString::from(format!(
                                                                "{}",
                                                                hue_value.round() as i64
                                                            ))),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .h(px(22.0))
                                                    .w_full()
                                                    .rounded(px(6.0))
                                                    .bg(theme.accent)
                                                    .opacity(if theme.is_dark_mode {
                                                        0.72
                                                    } else {
                                                        0.82
                                                    })
                                                    .border_1()
                                                    .border_color(theme.vm_border),
                                            ),
                                    )
                                    .child(divider(&theme))
                                    // Support zone.
                                    .child(
                                        div()
                                            .rounded(px(10.0))
                                            .bg(theme.vm_panel)
                                            .border_1()
                                            .border_color(theme.vm_border)
                                            .p(px(10.0))
                                            .flex()
                                            .flex_col()
                                            .gap(px(8.0))
                                            .child(section_label("Support zone"))
                                            .child(meta_label(
                                                "Reset app configuration or export the full activity log for troubleshooting.",
                                            ))
                                            .child(
                                                div()
                                                    .flex()
                                                    .gap(px(10.0))
                                                    .child(div().flex_1().child(panel_button(
                                                        "export-log",
                                                        "Export Activity Log",
                                                        ButtonVariant::Soft,
                                                        true,
                                                        &theme,
                                                        self,
                                                        cx,
                                                        |this, _w, cx| {
                                                            this.prompt_export_activity_log(cx);
                                                        },
                                                    )))
                                                    .child(div().flex_1().child(panel_button(
                                                        "reset-config",
                                                        "Reset Configuration",
                                                        ButtonVariant::Danger,
                                                        true,
                                                        &theme,
                                                        self,
                                                        cx,
                                                        |this, w, cx| {
                                                            this.take_modal_focus(w, cx);
                                                            this.confirm_enter_at = Some(std::time::Instant::now());
                                                            this.confirm_exit_at = None;
                                                            this.confirm = Some(ConfirmState {
                                                                title: "Reset Configuration".to_string(),
                                                                message: if this.show_dev_details() {
                                                                    "This removes saved settings, the projects folder choice, and project notes.\n\nProject version history (.musit folders) is not deleted.".to_string()
                                                                } else {
                                                                    "This clears your settings, folder choices, and project notes.\n\nYour saved versions are kept.".to_string()
                                                                },
                                                                confirm_text: "Reset".to_string(),
                                                                danger: true,
                                                                action: ConfirmAction::ResetConfig,
                                                            });
                                                            cx.notify();
                                                        },
                                                    ))),
                                            ),
                                    ),
                            ),
                    )
                    .child(scrollbar(
                        "settings",
                        &self.settings_scroll.clone(),
                        ScrollAxis::Vertical,
                        &theme,
                        cx,
                    )),
            );

        self.popup_frame(
            "settings-popup",
            (size.0, PopupHeight::Fixed(size.1)),
            content.into_any_element(),
            self.settings_enter_at,
            self.settings_exit_at,
            cx,
        )
    }

    // ---- Projects-folder layout dialog --------------------------------------

    fn render_layout_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let size = popup_size(window, (460.0, 240.0), (600.0, 620.0), (560.0, 520.0));

        let folder_path = self.layout_dialog.folder_path.clone();
        let selected = self.layout_dialog.selected_layout.clone();
        let is_new_folder = self.layout_dialog.is_new_folder;

        // Radio dot: the border alone left both cards looking equally chosen.
        let radio = move |selected: bool| {
            div()
                .flex_none()
                .mt(px(2.0))
                .w(px(16.0))
                .h(px(16.0))
                .rounded_full()
                .border_1()
                .border_color(if selected {
                    theme.modal_selected_border
                } else {
                    theme.modal_option_border
                })
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |el| {
                    el.child(
                        div()
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded_full()
                            .bg(theme.modal_selected_border),
                    )
                })
        };

        let card = |id: &'static str,
                    layout: &'static str,
                    title: &'static str,
                    description: &'static str,
                    selected: bool,
                    cx: &mut Context<RootView>| {
            div()
                .id(id)
                .w_full()
                .rounded(px(MODAL_PANEL_RADIUS))
                .bg(if selected {
                    theme.modal_option_selected_fill
                } else {
                    theme.modal_option_fill
                })
                .border_1()
                .border_color(if selected {
                    theme.modal_selected_border
                } else {
                    theme.modal_option_border
                })
                .px(px(13.0))
                .py(px(12.0))
                .flex()
                .items_start()
                .gap(px(11.0))
                .when(!selected, |el| {
                    el.hover(move |style| style.bg(theme.modal_option_fill_hover))
                })
                .cursor_pointer()
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.layout_dialog_select(layout);
                    cx.notify();
                }))
                .child(radio(selected))
                .child(
                    // min_w_0: without it the flex item takes its min-content
                    // width and the description runs past the card edge
                    // instead of wrapping.
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(3.0))
                        .child(
                            div()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.vm_text_primary)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(px(12.0))
                                .line_height(px(16.0))
                                .text_color(theme.vm_text_meta)
                                .child(description),
                        ),
                )
        };

        // Nothing happens until OK, so the footer just says what OK will do.
        let footer_text: SharedString = if self.show_dev_details() {
            "Saved to .immersion/settings.json in this folder.".into()
        } else if is_new_folder {
            "The folder is scanned once you press OK.".into()
        } else {
            "Changing this rescans the folder.".into()
        };

        let body = div()
            .id("layout-dialog-scroll")
            .w_full()
            .max_h(px(size.1))
            .overflow_y_scroll()
            .track_scroll(&self.layout_dialog_scroll)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.0))
                    .px(px(20.0))
                    .py(px(18.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(18.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.vm_text_primary)
                                    .child("How are your projects organized?"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .line_height(px(16.0))
                                    .text_color(theme.vm_text_meta)
                                    .child("Pick the option that matches this folder."),
                            )
                            .when(!folder_path.is_empty(), |el| {
                                el.child(
                                    div()
                                        .w_full()
                                        .text_size(px(11.0))
                                        .text_color(theme.vm_text_meta)
                                        .opacity(0.7)
                                        .truncate()
                                        .child(SharedString::from(folder_path)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(card(
                                "layout-card-bundles",
                                "Bundles",
                                "Project folders with bundles",
                                "Each project has its own subfolder or bundle, like .logicx.",
                                selected == "Bundles",
                                cx,
                            ))
                            .child(card(
                                "layout-card-files",
                                "Files",
                                "Loose project files",
                                "Project files like .als or .flp sit directly in the folder.",
                                selected == "Files",
                                cx,
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(11.5))
                                    .line_height(px(15.0))
                                    .text_color(theme.vm_text_meta)
                                    .opacity(0.7)
                                    .child(footer_text),
                            )
                            .child(div().w(px(88.0)).flex_none().child(panel_button(
                                "layout-dialog-cancel",
                                "Cancel",
                                ButtonVariant::Soft,
                                true,
                                &theme,
                                self,
                                cx,
                                |this, _w, cx| {
                                    this.request_close_layout(cx);
                                },
                            )))
                            .child(div().w(px(88.0)).flex_none().child(panel_button(
                                "layout-dialog-ok",
                                "OK",
                                ButtonVariant::Primary,
                                true,
                                &theme,
                                self,
                                cx,
                                |this, _w, cx| {
                                    this.layout_dialog_confirm(cx);
                                },
                            ))),
                    ),
            );

        self.popup_frame(
            "layout-dialog",
            (size.0, PopupHeight::FitContent(size.1)),
            body.into_any_element(),
            self.layout_enter_at,
            self.layout_exit_at,
            cx,
        )
    }

    // ---- Confirm dialog -----------------------------------------------------

    fn render_confirm_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let size = popup_size(window, (420.0, 230.0), (520.0, 320.0), (480.0, 260.0));
        let (title, message, confirm_text, danger) = self
            .confirm
            .as_ref()
            .map(|confirm| {
                (
                    confirm.title.clone(),
                    confirm.message.clone(),
                    confirm.confirm_text.clone(),
                    confirm.danger,
                )
            })
            .unwrap_or_default();

        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(18.0))
            .gap(px(14.0))
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.vm_text_primary)
                    .child(SharedString::from(title)),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(13.0))
                    .line_height(px(17.5))
                    .text_color(theme.vm_text_meta)
                    .child(SharedString::from(message)),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(10.0))
                    .child(div().w(px(96.0)).child(panel_button(
                        "confirm-cancel",
                        "Cancel",
                        ButtonVariant::Soft,
                        true,
                        &theme,
                        self,
                        cx,
                        |this, _w, cx| {
                            this.request_close_confirm(cx);
                        },
                    )))
                    .child(div().w(px(112.0)).child(panel_button(
                        "confirm-accept",
                        &confirm_text,
                        if danger {
                            ButtonVariant::Danger
                        } else {
                            ButtonVariant::Primary
                        },
                        true,
                        &theme,
                        self,
                        cx,
                        |this, _w, cx| {
                            this.run_confirm_action(cx);
                        },
                    ))),
            );

        self.popup_frame(
            "confirm-dialog",
            (size.0, PopupHeight::Fixed(size.1)),
            content.into_any_element(),
            self.confirm_enter_at,
            self.confirm_exit_at,
            cx,
        )
    }

    // ---- File dialogs --------------------------------------------------------

    pub fn prompt_for_projects_folder(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select Projects Folder".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.first() {
                    let folder = path.to_string_lossy().to_string();
                    this.update(cx, |root: &mut RootView, cx| {
                        // A folder set up before keeps its type in its own
                        // settings.json — add it straight away instead of
                        // asking again.
                        if musit_core::folder_settings::has_layout_setting(&folder) {
                            root.pending_select_latest = true;
                            root.backend.add_projects_folder(&folder, "");
                        } else {
                            root.open_layout_dialog_for_folder(&folder);
                        }
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
    }

    pub fn prompt_export_activity_log(&mut self, cx: &mut Context<Self>) {
        let folder = self.backend.default_activity_log_export_folder();
        // The backend returns a full path; the prompt wants dir + file name.
        let default_file = self.backend.default_activity_log_export_file();
        let file_name = std::path::Path::new(&default_file)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "Immersion Logs.log".to_string());
        let receiver = cx.prompt_for_new_path(std::path::Path::new(&folder), Some(&file_name));
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = receiver.await {
                let path_string = path.to_string_lossy().to_string();
                this.update(cx, |root: &mut RootView, cx| {
                    root.backend.export_activity_log(&path_string);
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
}

/// Delete-version confirm content (VersionManagerWindow deleteConfirmDialog).
pub fn delete_version_confirm(version_id: &str, dev_details: bool) -> ConfirmState {
    ConfirmState {
        title: "Delete Version".to_string(),
        message: if dev_details {
            format!(
                "Delete version v{version_id}?\n\nThis removes its snapshot from .musit and cannot be undone."
            )
        } else {
            format!("Delete version v{version_id}?\n\nThis cannot be undone.")
        },
        confirm_text: "Delete".to_string(),
        danger: true,
        action: ConfirmAction::DeleteVersion(version_id.to_string()),
    }
}
