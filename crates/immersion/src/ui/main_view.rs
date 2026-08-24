//! Main view: status row, folder tabs, a 1:1 split of project list (left)
//! and version graph (right), and a details panel (blame/notes) at the
//! bottom (~0.3 of the split area, draggable).

use gpui::{
    Context, ElementId, FontWeight, MouseButton, MouseDownEvent, SharedString, Window, canvas, div,
    linear_color_stop, linear_gradient, prelude::*, px, relative, uniform_list,
};

use crate::app::{
    ComboId, ConfirmAction, ConfirmState, GRAPH_PANE_MIN, LIST_PANE_MIN, RootView,
};
use crate::ui::controls::{ScrollAxis, action_button, scrollbar, text_field};

/// Uniform row pitch: 72px card + 8px spacing baked into the row.
const PROJECT_ROW_PITCH: f32 = 80.0;
/// Shared height of everything in the header row: tabs, "+", Settings.
const TAB_HEIGHT: f32 = 30.0;

impl RootView {
    pub fn render_main_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        // Height from the split bounds captured last frame (the canvas below
        // re-notifies when they change, so a stale value lasts one frame),
        // clamped so the top row always keeps ≥180px and the panel ≥150px.
        // Same bounds the details drag uses: the top row spans the split area.
        let list_width = self.split_bounds.map(|bounds| {
            let total = f32::from(bounds.size.width);
            // Re-clamped every frame, so narrowing the window can't push
            // either pane under its floor either.
            (total * self.list_fraction).clamp(LIST_PANE_MIN, (total - GRAPH_PANE_MIN).max(LIST_PANE_MIN))
        });
        let details_height = self
            .split_bounds
            .map(|bounds| {
                let split_height = f32::from(bounds.size.height);
                (split_height * self.details_fraction)
                    .clamp(210.0, (split_height - 200.0).max(210.0))
            })
            .unwrap_or(220.0);

        div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(16.0))
            .gap(px(10.0))
            // Header: folder tabs (each carrying its own scan progress) and
            // Settings. There is no status line — a tab says what it is doing.
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(12.0))
                    .child(
                        // One row however many folders there are: the strip
                        // scrolls sideways instead of wrapping and pushing the
                        // panels down. The tabs are direct children of the
                        // scroll container — wrapped in a row they get clamped
                        // to its width and there is nothing left to scroll.
                        div()
                            .flex_1()
                            .min_w_0()
                            .relative()
                            .child(
                                div()
                                    .id("folder-tabs-scroll")
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    // Room under the tabs for the scrollbar.
                                    .pb(px(8.0))
                                    .overflow_x_scroll()
                                    .track_scroll(&self.tabs_scroll)
                                    .children(self.folder_tab_items(cx)),
                            )
                            .children(self.tab_strip_fades())
                            .child(scrollbar(
                                "folder-tabs",
                                &self.tabs_scroll.clone(),
                                ScrollAxis::Horizontal,
                                &theme,
                                cx,
                            )),
                    )
                    .child(self.render_add_folder_button(cx))
                    .child(
                        // Tab height, so the three header controls line up on
                        // both edges instead of stepping.
                        div()
                            .w(px(108.0))
                            .h(px(TAB_HEIGHT))
                            .flex_none()
                            .child(action_button(
                                "settings-button",
                                "Settings",
                                &theme,
                                self,
                                cx,
                                |this, w, cx| {
                                    this.open_settings(w, cx);
                                    cx.notify();
                                },
                            )),
                    ),
            )
            // Split area: [list | graph] over details.
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .relative()
                    .overflow_hidden()
                    .child({
                        // Capture split-area bounds for the drag handle and
                        // details-height math; re-render once when they change
                        // (resize) so the panel never keeps a stale height.
                        let weak = cx.entity().downgrade();
                        canvas(
                            move |bounds, _window, cx| {
                                if let Some(root) = weak.upgrade() {
                                    root.update(cx, |root, cx| {
                                        if root.split_bounds != Some(bounds) {
                                            root.split_bounds = Some(bounds);
                                            cx.notify();
                                        }
                                    });
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full()
                    })
                    // Top row: project list | graph, split on a draggable
                    // divider (the list keeps `list_fraction` of the width).
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(
                                div()
                                    .map(|el| match list_width {
                                        Some(width) => el.w(px(width)),
                                        None => el.w(relative(self.list_fraction)),
                                    })
                                    .flex_none()
                                    .min_w(px(LIST_PANE_MIN))
                                    .h_full()
                                    .child(self.render_projects_panel(window, cx)),
                            )
                            .child(
                                div()
                                    .id("list-split-handle")
                                    .w(px(10.0))
                                    .h_full()
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor(gpui::CursorStyle::ResizeColumn)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _event, _window, cx| {
                                            this.list_split_dragging = true;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        div()
                                            .w(px(6.0))
                                            .h(px(6.0))
                                            .rounded(px(3.0))
                                            .bg(theme.text_secondary)
                                            .opacity(if self.list_split_dragging {
                                                1.0
                                            } else {
                                                0.5
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(self.render_graph_panel(cx)),
                            ),
                    )
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
                    // Details panel (blame, description, notes).
                    .child(
                        div()
                            .h(px(details_height))
                            .w_full()
                            .flex_none()
                            .child(self.render_details_panel(window, cx)),
                    ),
            )
    }

    fn folder_tab_items(&mut self, cx: &mut Context<RootView>) -> Vec<gpui::AnyElement> {
        let theme = self.theme;
        let tabs = self.backend.folder_tabs();
        let active = self.backend.active_folder_index();
        let renaming = self.renaming_tab;

        tabs.iter()
            .enumerate()
            .map(|(index, tab)| {
                let selected = index as i32 == active;
                let is_renaming = renaming == Some(index as i32);
                let label = tab.name.clone();
                let confirm_name = tab.name.clone();
                let status = tab.status.clone();
                let tab_path = tab.path.clone();
                let tab_name = tab.name.clone();
                div()
                    .id(ElementId::NamedInteger("folder-tab".into(), index as u64))
                    .h(px(TAB_HEIGHT))
                    .flex_none()
                    .max_w(px(260.0))
                    .pl(px(12.0))
                    .pr(px(6.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .cursor_pointer()
                    .bg(if selected {
                        theme.panel_surface
                    } else {
                        theme.row_odd
                    })
                    .border_1()
                    .border_color(if selected {
                        theme.accent
                    } else {
                        theme.node_border
                    })
                    // Click selects; double-click renames inline.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                            if this.renaming_tab == Some(index as i32) {
                                return;
                            }
                            if event.click_count >= 2 {
                                this.renaming_tab = Some(index as i32);
                                let name = tab_name.clone();
                                this.tab_name_input
                                    .update(cx, |input, cx| input.set_text(&name, cx));
                                let focus =
                                    this.tab_name_input.read(cx).focus_handle.clone();
                                window.focus(&focus, cx);
                            } else {
                                this.renaming_tab = None;
                                this.pending_select_latest = true;
                                this.backend.set_active_folder_index(index as i32);
                            }
                            cx.notify();
                        }),
                    )
                    .when(is_renaming, |el| {
                        el.child(
                            div()
                                .w(px(150.0))
                                .text_size(px(12.0))
                                .child(self.tab_name_input.clone()),
                        )
                    })
                    .when(!is_renaming, |el| {
                        el.child(
                            div()
                                .text_size(px(12.0))
                                .font_weight(if selected {
                                    FontWeight::BOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .text_color(theme.text_primary)
                                .child(SharedString::from(label)),
                        )
                        .when(!status.is_empty(), |el| {
                            el.child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme.text_secondary)
                                    .child(SharedString::from(status)),
                            )
                        })
                    })
                    // Close: stop watching this folder (history stays on disk).
                    .child(
                        div()
                            .id(ElementId::NamedInteger("folder-tab-close".into(), index as u64))
                            .w(px(18.0))
                            .h(px(18.0))
                            .rounded(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.0))
                            .text_color(theme.text_secondary)
                            .hover(move |style| style.bg(theme.button_soft_fill))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                                    cx.stop_propagation();
                                    this.take_modal_focus(window, cx);
                                    this.confirm_enter_at = Some(std::time::Instant::now());
                                    this.confirm_exit_at = None;
                                    this.confirm = Some(ConfirmState {
                                        title: "Remove Projects Folder".to_string(),
                                        message: if this.show_dev_details() {
                                            format!(
                                                "Stop watching and versioning {tab_path}?\n\nExisting version history (.musit folders) is not deleted."
                                            )
                                        } else {
                                            format!(
                                                "Stop watching {confirm_name} for changes?\n\nNothing is deleted — its saved versions stay, and you can add the folder back later."
                                            )
                                        },
                                        confirm_text: "Remove".to_string(),
                                        danger: true,
                                        action: ConfirmAction::RemoveFolder(index as i32),
                                    });
                                    cx.notify();
                                }),
                            )
                            .child("\u{2715}"),
                    )
                    .into_any_element()
            })
            .collect()
    }

    /// Fades at whichever end of the tab strip has more tabs beyond it, so a
    /// clipped tab reads as "there is more" rather than as a broken layout.
    /// Plain divs: no id, no handlers, so clicks land on the tabs underneath.
    fn tab_strip_fades(&self) -> Vec<gpui::AnyElement> {
        const FADE_WIDTH: f32 = 40.0;

        let scrolled = -f32::from(self.tabs_scroll.offset().x);
        let max_scroll = f32::from(self.tabs_scroll.max_offset().x);
        let opaque: gpui::Hsla = gpui::Rgba::from(self.theme.app_background).into();
        let clear = opaque.opacity(0.0);

        let mut fades = Vec::new();
        if scrolled > 0.5 {
            fades.push(
                div()
                    .absolute()
                    .top_0()
                    .bottom(px(6.0))
                    .left_0()
                    .w(px(FADE_WIDTH))
                    .bg(linear_gradient(
                        90.0,
                        linear_color_stop(opaque, 0.0),
                        linear_color_stop(clear, 1.0),
                    ))
                    .into_any_element(),
            );
        }
        if max_scroll - scrolled > 0.5 {
            fades.push(
                div()
                    .absolute()
                    .top_0()
                    .bottom(px(6.0))
                    .right_0()
                    .w(px(FADE_WIDTH))
                    .bg(linear_gradient(
                        90.0,
                        linear_color_stop(clear, 0.0),
                        linear_color_stop(opaque, 1.0),
                    ))
                    .into_any_element(),
            );
        }
        fades
    }

    /// The "+" that adds another projects folder. It sits outside the tab
    /// strip so it stays reachable once the strip starts scrolling.
    fn render_add_folder_button(&mut self, cx: &mut Context<RootView>) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id("folder-tab-add")
            .h(px(TAB_HEIGHT))
            .w(px(TAB_HEIGHT))
            .flex_none()
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(theme.row_odd)
            .border_1()
            .border_color(theme.node_border)
            .hover(move |style| style.bg(theme.button_soft_fill))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseDownEvent, _window, cx| {
                    this.prompt_for_projects_folder(cx);
                }),
            )
            .child(
                // Two bars, not a "+" glyph: the typeset plus sits below the
                // optical centre of the box.
                div()
                    .relative()
                    .w(px(12.0))
                    .h(px(12.0))
                    .child(
                        div()
                            .absolute()
                            .top(px(5.0))
                            .left_0()
                            .w(px(12.0))
                            .h(px(2.0))
                            .rounded(px(1.0))
                            .bg(theme.text_secondary),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(5.0))
                            .top_0()
                            .w(px(2.0))
                            .h(px(12.0))
                            .rounded(px(1.0))
                            .bg(theme.text_secondary),
                    ),
            )
    }

    fn render_projects_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let project_count = self.backend.projects().len();
        let has_projects = project_count > 0;
        let is_scanning = self.backend.is_scanning_projects();
        let sort_options = vec!["Name".to_string(), "Last Opened".to_string()];
        let sort_index = if self.backend.sort_mode() == "Last Opened" {
            1
        } else {
            0
        };

        div()
            .size_full()
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
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.0))
                            .text_color(theme.text_primary)
                            .child("Discovered Projects"),
                    )
                    .child(div().flex_1().min_w(px(8.0)))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .flex_none()
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
            // Project list (virtualized: only visible rows are built).
            // Click selects; double-click opens the project.
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(
                        uniform_list(
                            "project-list",
                            project_count,
                            cx.processor(move |this: &mut RootView, range: std::ops::Range<usize>, _window, cx| {
                                let theme = this.theme;
                                let selected_index = this.backend.selected_project_index();
                                range
                                    .map(|index| {
                                        let Some(project) = this.backend.projects().get(index)
                                        else {
                                            return div().into_any_element();
                                        };
                                        let row_color = if index % 2 == 0 {
                                            theme.row_even
                                        } else {
                                            theme.row_odd
                                        };
                                        let selected = index as i32 == selected_index;
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
                                        let file_line = format!("Project file: {}", project.file);
                                        let path_line = format!("Path: {}", project.path);
                                        div()
                                            .h(px(PROJECT_ROW_PITCH))
                                            .w_full()
                                            .pb(px(8.0))
                                            .child(
                                                div()
                                                    .id(ElementId::NamedInteger(
                                                        "project-row".into(),
                                                        index as u64,
                                                    ))
                                                    .size_full()
                                                    .rounded(px(6.0))
                                                    .bg(row_color)
                                                    // Constant border width: only the
                                                    // color changes on selection, so
                                                    // content never shifts.
                                                    .border_2()
                                                    .border_color(if selected {
                                                        theme.accent
                                                    } else {
                                                        row_color
                                                    })
                                                    .p(px(8.0))
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(10.0))
                                                    .cursor_pointer()
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |this, event: &MouseDownEvent, _w, cx| {
                                                                if event.click_count >= 2 {
                                                                    this.backend
                                                                        .open_project(index as i32);
                                                                } else {
                                                                    this.select_project(
                                                                        index as i32,
                                                                        cx,
                                                                    );
                                                                }
                                                                cx.notify();
                                                            },
                                                        ),
                                                    )
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
                                                                    .text_size(px(12.0))
                                                                    .text_color(theme.text_secondary)
                                                                    .truncate()
                                                                    .child(SharedString::from(file_line)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(12.0))
                                                                    .text_color(theme.text_muted)
                                                                    .truncate()
                                                                    .child(SharedString::from(path_line)),
                                                            ),
                                                    ),
                                            )
                                            .into_any_element()
                                    })
                                    .collect()
                            }),
                        )
                        .size_full()
                        .track_scroll(&self.project_list_scroll),
                    )
                    .child(scrollbar(
                        "project-list",
                        &self.project_list_scroll.0.borrow().base_handle.clone(),
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

    /// Activity log list, shown inside Settings.
    pub fn render_activity_log(&mut self, cx: &mut Context<RootView>) -> impl IntoElement {
        let theme = self.theme;
        let count = self.backend.activity().len();

        div()
            .size_full()
            .relative()
            .child(
                uniform_list(
                    "activity-list",
                    count,
                    cx.processor(
                        move |this: &mut RootView, range: std::ops::Range<usize>, _window, _cx| {
                            range
                                .map(|index| {
                                    div()
                                        .id(ElementId::NamedInteger(
                                            "activity-row".into(),
                                            index as u64,
                                        ))
                                        .w_full()
                                        .h(px(16.0))
                                        .line_height(px(16.0))
                                        .truncate()
                                        .child(SharedString::from(
                                            this.backend
                                                .activity()
                                                .get(index)
                                                .cloned()
                                                .unwrap_or_default(),
                                        ))
                                })
                                .collect()
                        },
                    ),
                )
                .size_full()
                .text_size(px(12.0))
                .text_color(theme.text_activity)
                .track_scroll(&self.activity_scroll),
            )
            .child(scrollbar(
                "activity",
                &self.activity_scroll.0.borrow().base_handle.clone(),
                ScrollAxis::Vertical,
                &theme,
                cx,
            ))
    }
}
