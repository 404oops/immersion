//! MainView.ui.qml: status row, project list panel, activity panel,
//! with a draggable vertical split between the two panels.

use gpui::{
    Context, ElementId, FontWeight, MouseButton, SharedString, Window, canvas, div, prelude::*,
    px, uniform_list,
};

use crate::app::{ComboId, RootView};
use crate::ui::controls::{ScrollAxis, action_button, scrollbar, text_field};

impl RootView {
    pub fn render_main_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;

        div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(16.0))
            .gap(px(10.0))
            // Status row.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.0))
                            .text_color(theme.text_status)
                            .child(SharedString::from(
                                self.backend.status_message().to_string(),
                            )),
                    )
                    .child(
                        div().w(px(120.0)).h(px(36.0)).flex_none().child(
                            action_button("settings-button", "Settings", &theme, self, cx, |this, w, cx| {
                                this.open_settings(w, cx);
                                cx.notify();
                            }),
                        ),
                    ),
            )
            // Split area.
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .child({
                        // Capture split-area bounds for the drag handle math.
                        let weak = cx.entity().downgrade();
                        canvas(
                            move |bounds, _window, cx| {
                                if let Some(root) = weak.upgrade() {
                                    root.update(cx, |root, _| {
                                        if root.split_bounds != Some(bounds) {
                                            root.split_bounds = Some(bounds);
                                        }
                                    });
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full()
                    })
                    .child(self.render_projects_panel(window, cx))
                    // Split handle: 20px tall, centered 6x6 dot.
                    .child(
                        div()
                            .id("split-handle")
                            .h(px(20.0))
                            .w_full()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor(gpui::CursorStyle::ResizeRow)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _event, _window, cx| {
                                    this.split_dragging = true;
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .w(px(6.0))
                                    .h(px(6.0))
                                    .rounded(px(3.0))
                                    .bg(theme.text_secondary)
                                    .opacity(if self.split_dragging { 1.0 } else { 0.5 }),
                            ),
                    )
                    .child(self.render_activity_panel(cx)),
            )
    }

    fn render_projects_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let projects = self.backend.projects().to_vec();
        let has_projects = !projects.is_empty();
        let is_scanning = self.backend.is_scanning_projects();
        let sort_options = vec!["Name".to_string(), "Last Opened".to_string()];
        let sort_index = if self.backend.sort_mode() == "Last Opened" { 1 } else { 0 };

        div()
            .flex_1()
            .min_h_0()
            .rounded(px(8.0))
            .bg(theme.panel_surface)
            .p(px(10.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            // Header row.
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(theme.text_primary)
                            .child("Discovered Projects"),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .text_color(theme.text_secondary)
                                    .child("Sort:"),
                            )
                            .child(self.render_combo(
                                ComboId::SortMain,
                                "sort-main",
                                sort_index,
                                &sort_options,
                                Some(150.0),
                                cx,
                                |this, index, _window, _cx| {
                                    let mode = if index == 1 { "Last Opened" } else { "Name" };
                                    this.backend.set_sort_mode(mode);
                                },
                            )),
                    ),
            )
            // Search field.
            .child(text_field(&self.search_input, &theme, window, cx))
            // Project list.
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        div()
                            .id("project-list")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.project_list_scroll)
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .children(projects.iter().enumerate().map(|(index, project)| {
                                let row_color = if index % 2 == 0 {
                                    theme.row_even
                                } else {
                                    theme.row_odd
                                };
                                let name = if project.name.is_empty() {
                                    format!("Project {}", index + 1)
                                } else {
                                    project.name.clone()
                                };
                                let kind = if project.kind.is_empty() {
                                    "DAW".to_string()
                                } else {
                                    project.kind.clone()
                                };
                                div()
                                    .id(ElementId::NamedInteger("project-row".into(), index as u64))
                                    .h(px(86.0))
                                    .w_full()
                                    .flex_none()
                                    .rounded(px(6.0))
                                    .bg(row_color)
                                    .p(px(8.0))
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    // Kind badge.
                                    .child(
                                        div()
                                            .w(px(48.0))
                                            .h(px(48.0))
                                            .flex_none()
                                            .rounded(px(6.0))
                                            .bg(theme.accent)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                div()
                                                    .w(px(42.0))
                                                    .text_size(px(10.0))
                                                    .text_color(gpui::white())
                                                    .text_center()
                                                    .child(SharedString::from(kind)),
                                            ),
                                    )
                                    // Metadata column.
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.0))
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme.text_primary)
                                                    .child(SharedString::from(name)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .text_color(theme.text_secondary)
                                                    .child(SharedString::from(format!(
                                                        "Project file: {}",
                                                        project.file
                                                    ))),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .text_color(theme.text_muted)
                                                    .truncate()
                                                    .child(SharedString::from(format!(
                                                        "Path: {}",
                                                        project.path
                                                    ))),
                                            ),
                                    )
                                    // Action column.
                                    .child(
                                        div()
                                            .w(px(210.0))
                                            .h_full()
                                            .flex_none()
                                            .flex()
                                            .gap(px(8.0))
                                            .child(
                                                div().flex_1().h_full().child(action_button(
                                                    ElementId::NamedInteger(
                                                        "open-project".into(),
                                                        index as u64,
                                                    ),
                                                    "Open",
                                                    &theme,
                                                    self,
                                                    cx,
                                                    move |this, _w, cx| {
                                                        this.backend.open_project(index as i32);
                                                        cx.notify();
                                                    },
                                                )),
                                            )
                                            .child(
                                                div().flex_1().h_full().child(action_button(
                                                    ElementId::NamedInteger(
                                                        "manage-project".into(),
                                                        index as u64,
                                                    ),
                                                    "Manage",
                                                    &theme,
                                                    self,
                                                    cx,
                                                    move |this, w, cx| {
                                                        this.open_version_manager(index as i32, w, cx);
                                                    },
                                                )),
                                            ),
                                    )
                            })),
                    )
                    .child(scrollbar(
                        "project-list",
                        &self.project_list_scroll.clone(),
                        ScrollAxis::Vertical,
                        &theme,
                        cx,
                    ))
                    .when(!has_projects, |el| {
                        el.child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(20.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.text_status)
                                .child(if is_scanning {
                                    "Scanning for projects\u{2026}"
                                } else {
                                    "No projects loaded"
                                }),
                        )
                    }),
            )
    }

    fn render_activity_panel(&mut self, cx: &mut Context<RootView>) -> impl IntoElement {
        let theme = self.theme;
        let activity: Vec<String> = self.backend.activity().to_vec();
        let count = activity.len();

        div()
            .h(px(self.activity_panel_height))
            .w_full()
            .flex_none()
            .rounded(px(8.0))
            .bg(theme.panel_surface)
            .p(px(10.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(theme.text_primary)
                    .child("Activity"),
            )
            .child(
                div().flex_1().min_h_0().relative().child(
                    uniform_list(
                        "activity-list",
                        count,
                        cx.processor(move |_this, range: std::ops::Range<usize>, _window, _cx| {
                            range
                                .map(|index| {
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "activity-row".into(),
                                            index as u64,
                                        ))
                                        .w_full()
                                        // Qt Label rows: ~16px line, no ListView spacing.
                                        .h(px(16.0))
                                        .line_height(px(16.0))
                                        .truncate()
                                        .child(SharedString::from(
                                            activity.get(index).cloned().unwrap_or_default(),
                                        ))
                                })
                                .collect()
                        }),
                    )
                    .size_full()
                    .text_size(px(13.0))
                    .text_color(theme.text_activity)
                    .track_scroll(&self.activity_scroll),
                )
                .child(scrollbar(
                    "activity",
                    &self.activity_scroll.0.borrow().base_handle.clone(),
                    ScrollAxis::Vertical,
                    &theme,
                    cx,
                )),
            )
    }
}
