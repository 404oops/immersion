//! Modal layer: the settings panel with its scrim, and the two vampir
//! dialogs — the projects-folder layout question and the confirm.

use gpui::{
    AnyElement, Context, ElementId, FontWeight, PathPromptOptions, SharedString, Window, div,
    prelude::*, px,
};

use crate::app::{ConfirmAction, ConfirmState, RootView};
use crate::theme::MODAL_EDGE_PADDING;
use crate::theme::MODAL_PANEL_RADIUS;
use crate::ui::controls::{
    ButtonVariant, CONFIRM_DIALOG, LAYOUT_DIALOG, ScrollAxis, SpinboxSettings, modal_opacity,
    panel_button, scrollbar, themed_spinbox, themed_switch,
};
use crate::ui::onboarding::{LAYOUTS, layout_choices};
use vampir::{DialogButton, dialog, radio_group};

/// Settings panel width/height rule.
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

        // The confirm dialog's question outlives the dialog by its fade, so
        // it has something to show on the way out; once the dialog has gone,
        // so has the question.
        if self.confirm.is_some() && self.controls.dialog_fade(CONFIRM_DIALOG).is_none() {
            self.confirm = None;
        }

        if self.settings_open {
            let (scrim_opacity, _) = modal_opacity(self.settings_enter_at, self.settings_exit_at);
            // Full-window scrim behind the panel. It occludes the root, so it
            // must forward the shared drag tracking (hue slider, scrollbars).
            layers.push(
                div()
                    .id("modal-scrim")
                    .absolute()
                    .inset_0()
                    .bg(theme.modal_scrim)
                    .opacity(scrim_opacity)
                    .occlude()
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
                    .into_any_element(),
            );
            layers.push(self.render_settings_modal(window, cx).into_any_element());
        }

        // The dialogs bring their own scrim and fade, and are deferred, so
        // one opened from the settings panel paints over it.
        layers.extend(self.render_layout_dialog(cx));
        layers.extend(self.render_confirm_dialog(cx));

        layers
    }

    /// Centers the settings panel; opacity fade on enter/exit applied once to
    /// the whole panel (chrome + content).
    fn popup_frame(
        &self,
        id: &'static str,
        size: (f32, f32),
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
                    .h(px(size.1))
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
        let saturation_percent = (self.backend.theme_saturation() * 100.0).round() as i64;

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
                                                    cx,
                                                    |this, _window, cx| {
                                                        this.open_layout_dialog_for_current_folder();
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
                                                        ))
                                                        .when(
                                                            launch_at_startup
                                                                && crate::platform::launch_at_startup_needs_approval(),
                                                            |el| {
                                                                el.child(meta_label(
                                                                    "Allow Immersion in System Settings > General > Login Items & Extensions.",
                                                                ))
                                                            },
                                                        ),
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
                                                        SpinboxSettings {
                                                            value: retention,
                                                            min: 1,
                                                            max: 50,
                                                            enabled: true,
                                                        },
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
                                            .child(section_label("Theme saturation"))
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(12.0))
                                                    .child(self.render_saturation_slider(cx))
                                                    .child(
                                                        div()
                                                            .w(px(42.0))
                                                            .flex_none()
                                                            .text_size(px(12.0))
                                                            .text_color(theme.vm_text_meta)
                                                            .text_right()
                                                            .child(SharedString::from(format!(
                                                                "{saturation_percent}%"
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
                                                        cx,
                                                        |this, _w, cx| {
                                                            let message = if this.show_dev_details() {
                                                                "This removes saved settings, the projects folder choice, and project notes.\n\nProject version history (.musit folders) is not deleted."
                                                            } else {
                                                                "This clears your settings, folder choices, and project notes.\n\nYour saved versions are kept."
                                                            };
                                                            this.open_confirm(ConfirmState {
                                                                title: "Reset Configuration".to_string(),
                                                                message: message.to_string(),
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
                        self,
                        cx,
                    )),
            );

        self.popup_frame(
            "settings-popup",
            size,
            content.into_any_element(),
            self.settings_enter_at,
            self.settings_exit_at,
            cx,
        )
    }

    // ---- Projects-folder layout dialog --------------------------------------

    /// The layout question as a vampir dialog, while it is open or fading
    /// out; `None` otherwise. Picking an option only changes the selection;
    /// OK is what saves it and scans.
    fn render_layout_dialog(&self, cx: &mut Context<RootView>) -> Option<AnyElement> {
        let palette = self.theme.palette();
        let folder_path = self.layout_dialog.folder_path.clone();
        let selected = LAYOUTS
            .iter()
            .position(|(layout, _, _)| *layout == self.layout_dialog.selected_layout)
            .unwrap_or(0);
        let choices = layout_choices();

        // Nothing happens until OK, so the footer just says what OK will do.
        let footer_text: SharedString = if self.show_dev_details() {
            "Saved to .immersion/settings.json in this folder.".into()
        } else if self.layout_dialog.is_new_folder {
            "The folder is scanned once you press OK.".into()
        } else {
            "Changing this rescans the folder.".into()
        };

        let body = div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .line_height(px(16.0))
                            .text_color(palette.text_secondary)
                            .child("Pick the option that matches this folder."),
                    )
                    .when(!folder_path.is_empty(), |el| {
                        el.child(
                            div()
                                .w_full()
                                .text_size(px(11.0))
                                .text_color(palette.text_secondary)
                                .opacity(0.7)
                                .truncate()
                                .child(SharedString::from(folder_path)),
                        )
                    }),
            )
            .child(radio_group(
                "layout-choice",
                &choices,
                selected,
                true,
                self.widget_context(cx),
                |this, index, _window, cx| {
                    this.layout_dialog_select(LAYOUTS[index].0);
                    cx.notify();
                },
            ))
            .child(
                div()
                    .text_size(px(11.5))
                    .line_height(px(15.0))
                    .text_color(palette.text_secondary)
                    .opacity(0.7)
                    .child(footer_text),
            );

        dialog(
            LAYOUT_DIALOG,
            "How are your projects organized?",
            520.0,
            self.widget_context(cx),
            body,
            vec![
                DialogButton::new(
                    "layout-dialog-cancel",
                    "Cancel",
                    ButtonVariant::Soft,
                    |_this, _window, _cx| {},
                ),
                DialogButton::new(
                    "layout-dialog-ok",
                    "OK",
                    ButtonVariant::Primary,
                    |this: &mut RootView, _window, cx| {
                        this.layout_dialog_confirm();
                        cx.notify();
                    },
                ),
            ],
            |_this, _window, _cx| {},
        )
        .map(IntoElement::into_any_element)
    }

    // ---- Confirm dialog -----------------------------------------------------

    /// The confirm as a vampir dialog, while it is open or fading out.
    fn render_confirm_dialog(&self, cx: &mut Context<RootView>) -> Option<AnyElement> {
        let confirm = self.confirm.as_ref()?;
        let title = confirm.title.clone();
        let message = confirm.message.clone();
        let confirm_text = confirm.confirm_text.clone();
        let variant = if confirm.danger {
            ButtonVariant::Danger
        } else {
            ButtonVariant::Primary
        };
        let palette = self.theme.palette();

        let body = div()
            .text_size(px(13.0))
            .line_height(px(17.5))
            .text_color(palette.text_secondary)
            .child(SharedString::from(message));

        dialog(
            CONFIRM_DIALOG,
            &title,
            460.0,
            self.widget_context(cx),
            body,
            vec![
                DialogButton::new(
                    "confirm-cancel",
                    "Cancel",
                    ButtonVariant::Soft,
                    |_this, _window, _cx| {},
                ),
                DialogButton::new(
                    "confirm-accept",
                    confirm_text,
                    variant,
                    |this: &mut RootView, _window, cx| {
                        this.run_confirm_action(cx);
                    },
                ),
            ],
            |_this, _window, _cx| {},
        )
        .map(IntoElement::into_any_element)
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
            if let Ok(Ok(Some(paths))) = receiver.await
                && let Some(path) = paths.first()
            {
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
