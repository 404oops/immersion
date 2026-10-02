//! Main view: status row, folder tabs, a 1:1 split of project list (left)
//! and version graph (right), and a details panel (blame/notes) at the
//! bottom (~0.3 of the split area, draggable).

use chrono::{Datelike, Local, NaiveDateTime};
use gpui::{
    Context, ElementId, FontWeight, MouseButton, MouseDownEvent, ObjectFit, SharedString, Window,
    canvas, div, img, prelude::*, px, relative, uniform_list,
};

use crate::app::{ConfirmAction, ConfirmState, GRAPH_PANE_MIN, LIST_PANE_MIN, RootView};
use crate::ui::controls::{
    ButtonVariant, CONTROL_HEIGHT, CONTROL_RADIUS, DETAILS_SPLIT, FOLDER_TABS, LIST_SPLIT,
    ScrollAxis, caption, lerp_rgba, panel_button, scrollbar, text_field,
};
use crate::ui::lighting;
use vampir::color::with_alpha;
use vampir::{Tab, split_area, split_handle, tab_bar};

/// Uniform row pitch: name and file line plus padding; rows touch and are
/// separated by a hairline rather than by spacing.
const PROJECT_ROW_PITCH: f32 = 56.0;
/// Shared height of everything in the header row: tabs, "+", Settings.
const TAB_HEIGHT: f32 = CONTROL_HEIGHT;
/// Space between the header and the panels, and between the panels
/// themselves (both drag handles are exactly this thick).
const PANE_GAP: f32 = 12.0;

/// Two-letter monogram for a project kind: an all-caps short first word
/// ("FL Studio" -> "FL") is used as is, otherwise the first two letters.
fn kind_monogram(kind: &str) -> String {
    let first_word = kind.split_whitespace().next().unwrap_or("");
    if (1..=3).contains(&first_word.chars().count())
        && first_word
            .chars()
            .all(|c| c.is_uppercase() || c.is_ascii_digit())
    {
        return first_word.to_string();
    }
    let letters: String = kind
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(2)
        .collect();
    if letters.is_empty() {
        "?".to_string()
    } else {
        letters.to_uppercase()
    }
}

/// "2026-09-02 00:52:09" as "Sep 2", or "Sep 2, 2025" outside the current
/// year; anything unparseable is shown as is.
fn short_date(stamp: &str) -> String {
    let Ok(when) = NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M:%S") else {
        return stamp.to_string();
    };
    if when.year() == Local::now().year() {
        when.format("%b %-d").to_string()
    } else {
        when.format("%b %-d, %Y").to_string()
    }
}

/// Where a project sits inside its folder, when that says more than the
/// name does: None for projects directly in the folder (every row would
/// repeat the folder path otherwise) or in a folder of their own name.
fn location_hint(project_path: &str, folder_root: &str, name: &str) -> Option<String> {
    let rest = project_path
        .strip_prefix(folder_root)?
        .trim_start_matches(['/', '\\']);
    if rest.is_empty() || rest == name {
        None
    } else {
        Some(rest.to_string())
    }
}

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
            (total * self.list_fraction)
                .clamp(LIST_PANE_MIN, (total - GRAPH_PANE_MIN).max(LIST_PANE_MIN))
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
            .gap(px(PANE_GAP))
            // Header: folder tabs (each carrying its own scan progress) and
            // Settings. There is no status line — a tab says what it is doing.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(self.render_folder_tabs(cx))
                    .child(self.render_add_folder_button(cx))
                    .child(
                        // Tab height, so the three header controls line up on
                        // both edges instead of stepping.
                        div()
                            .w(px(92.0))
                            .h(px(TAB_HEIGHT))
                            .flex_none()
                            .child(panel_button(
                                "settings-button",
                                "Settings",
                                ButtonVariant::Soft,
                                true,
                                &theme,
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
                    // The two dividers read their position as a fraction of
                    // this same area, so both probes go here.
                    .child(split_area(LIST_SPLIT, cx))
                    .child(split_area(DETAILS_SPLIT, cx))
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
                            // The divider is the toolkit's; it reports where
                            // it was dragged to as a fraction of the split
                            // area (see `RootView::track_dragged`). Its grab
                            // area is narrower than the gap, so it is centred
                            // in it.
                            .child(
                                div()
                                    .w(px(PANE_GAP))
                                    .h_full()
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(split_handle(
                                        LIST_SPLIT,
                                        self.list_fraction,
                                        true,
                                        self.widget_context(cx),
                                    )),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .child(self.render_graph_panel(cx)),
                            ),
                    )
                    // Split handle: same gap as the list|graph one, so the
                    // three panels sit an even distance apart. Its fraction
                    // is the top row's share, measured from the top.
                    .child(
                        div()
                            .h(px(PANE_GAP))
                            .w_full()
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(split_handle(
                                DETAILS_SPLIT,
                                1.0 - self.details_fraction,
                                false,
                                self.widget_context(cx),
                            )),
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

    /// The folder tabs: the toolkit's tab bar, with each folder's scan
    /// status as its badge and a close on the active tab. Pressing selects,
    /// dragging reorders (the drop arrives at `RootView::tabs_reordered`),
    /// and a double-click renames inline, which the bar has no gesture of
    /// its own for: the press is read here, after the bar has seen it, and
    /// the name field is laid over the tab.
    fn render_folder_tabs(&mut self, cx: &mut Context<RootView>) -> impl IntoElement {
        let theme = self.theme;
        let folders = self.backend.folder_tabs();
        let active = usize::try_from(self.backend.active_folder_index()).unwrap_or(0);
        let tabs: Vec<Tab> = folders
            .iter()
            .enumerate()
            .map(|(index, folder)| {
                // Keyed by path, so two folders of one name keep their own
                // slide records.
                let mut tab = Tab::new(folder.name.clone()).id(folder.path.clone());
                if !folder.status.is_empty() {
                    tab = tab.badge(folder.status.clone());
                }
                // Close (stop watching the folder; history stays on disk) is
                // offered on the active tab only.
                if index == active {
                    tab = tab.closable();
                }
                tab
            })
            .collect();
        // Where the tab being renamed sits, relative to this row, from the
        // slots the bar recorded last frame.
        let renaming = self.renaming_tab.and_then(|index| {
            let slot = self
                .controls
                .slot(FOLDER_TABS, usize::try_from(index).ok()?)?;
            let row = self.controls.bounds((FOLDER_TABS, "row"))?;
            Some((slot, row))
        });
        let weak = cx.entity().downgrade();
        div()
            .flex_1()
            .min_w_0()
            .relative()
            .flex()
            .items_center()
            // Where this row is, so the rename field can be laid over the
            // tab it renames.
            .child(
                canvas(
                    move |bounds, _window, cx| {
                        if let Some(root) = weak.upgrade() {
                            root.update(cx, |root, _cx| {
                                root.controls.record_bounds((FOLDER_TABS, "row"), bounds);
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            // The second press of a double-click: the bar has just armed a
            // reorder from it, which is let go, and the name field takes over.
            // With no drag left, the bar's release does not count as a click,
            // so it neither re-selects the tab nor takes the keyboard back.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if event.click_count < 2 {
                        return;
                    }
                    let Some(index) = this.controls.tab_at(FOLDER_TABS, event.position) else {
                        return;
                    };
                    let Some(name) = this
                        .backend
                        .folder_tabs()
                        .get(index)
                        .map(|folder| folder.name.clone())
                    else {
                        return;
                    };
                    this.controls.end_drag();
                    this.renaming_tab = Some(index as i32);
                    this.tab_name_input
                        .update(cx, |input, cx| input.set_text(&name, cx));
                    let focus = this.tab_name_input.read(cx).focus_handle.clone();
                    window.focus(&focus, cx);
                    cx.notify();
                }),
            )
            .child(tab_bar(
                FOLDER_TABS,
                &tabs,
                active,
                self.widget_context(cx),
                |this, index, _window, cx| {
                    // A click on another tab ends a rename.
                    if this
                        .renaming_tab
                        .is_some_and(|renaming| renaming != index as i32)
                    {
                        this.renaming_tab = None;
                    }
                    this.pending_select_latest = true;
                    this.backend.set_active_folder_index(index as i32);
                    cx.notify();
                },
                |this, index, _window, cx| {
                    let Some(folder) = this.backend.folder_tabs().get(index).cloned() else {
                        return;
                    };
                    let message = if this.show_dev_details() {
                        format!(
                            "Stop watching and versioning {}?\n\nExisting version history (.musit folders) is not deleted.",
                            folder.path
                        )
                    } else {
                        format!(
                            "Stop watching {} for changes?\n\nNothing is deleted — its saved versions stay, and you can add the folder back later.",
                            folder.name
                        )
                    };
                    this.open_confirm(ConfirmState {
                        title: "Remove Projects Folder".to_string(),
                        message,
                        confirm_text: "Remove".to_string(),
                        danger: true,
                        action: ConfirmAction::RemoveFolder(index as i32),
                    });
                    cx.notify();
                },
            ))
            // The name field, over the tab it renames. Enter commits (see
            // `RootView::new`), Escape cancels (see `RootView::dismiss`).
            .when_some(renaming, |el, (slot, row)| {
                el.child(
                    div()
                        .absolute()
                        .left(px(f32::from(slot.left()) - f32::from(row.left())))
                        .top(px(f32::from(slot.top()) - f32::from(row.top())))
                        .w(px(f32::from(slot.size.width).max(150.0)))
                        .h(px(f32::from(slot.size.height)))
                        .px(px(8.0))
                        .flex()
                        .items_center()
                        .rounded(px(CONTROL_RADIUS))
                        .bg(theme.input_surface)
                        .border_1()
                        .border_color(theme.input_border_accent)
                        .text_size(px(12.0))
                        .child(self.tab_name_input.clone()),
                )
            })
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
            .rounded(px(CONTROL_RADIUS))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(lighting::lit(theme.button_soft_fill, 0.08))
            .border_1()
            .border_color(lighting::rim(theme.button_soft_fill, theme.is_dark_mode))
            .shadow(lighting::raised(theme.is_dark_mode))
            .hover(move |style| style.bg(lighting::lit(theme.button_soft_fill_hover, 0.1)))
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
                            .h(px(1.5))
                            .rounded(px(1.0))
                            .bg(theme.text_secondary),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(5.25))
                            .top_0()
                            .w(px(1.5))
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
            .bg(lighting::lit(theme.panel_surface, 0.02))
            .border_1()
            .border_color(theme.vm_border)
            .shadow(lighting::panel(theme.is_dark_mode))
            .p(px(10.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            // Header row: title with the count, sort control on the right.
            .child(
                div()
                    .h(px(CONTROL_HEIGHT))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .items_baseline()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.text_primary)
                                    .child("Projects"),
                            )
                            .when(has_projects, |el| {
                                el.child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(theme.text_muted)
                                        .child(SharedString::from(project_count.to_string())),
                                )
                            }),
                    )
                    .child(div().flex_1().min_w(px(8.0)))
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(8.0))
                            .child(caption(&theme, "Sort by"))
                            .child(self.render_combo(
                                "sort-main",
                                sort_index,
                                &sort_options,
                                Some(124.0),
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
                                let folder_root = this
                                    .backend
                                    .folder_tabs()
                                    .get(this.backend.active_folder_index().max(0) as usize)
                                    .map(|tab| tab.path.clone())
                                    .unwrap_or_default();
                                // Selection fills the row with the accent and
                                // turns its text white; hover is a faint tint.
                                let hover_bg = if theme.is_dark_mode {
                                    theme.row_even
                                } else {
                                    theme.row_odd
                                };
                                let on_accent = theme.button_label;
                                // Warm the icon cache for the rows about to be
                                // built: one OS lookup per new extension.
                                let files: Vec<String> = range
                                    .clone()
                                    .filter_map(|index| {
                                        this.backend.projects().get(index).map(|p| p.file.clone())
                                    })
                                    .collect();
                                for file in &files {
                                    this.file_icon(file);
                                }
                                range
                                    .map(|index| {
                                        let Some(project) = this.backend.projects().get(index)
                                        else {
                                            return div().into_any_element();
                                        };
                                        let selected = index as i32 == selected_index;
                                        // 0 = plain row, 1 = fully selected; in
                                        // between while a selection cross-fades.
                                        let weight = this
                                            .row_fade
                                            .as_ref()
                                            .map(|fade| fade.weight(&(index as i32), selected))
                                            .unwrap_or(if selected { 1.0 } else { 0.0 });
                                        let row_bg =
                                            lerp_rgba(theme.panel_surface, theme.button_fill, weight);
                                        let name_color =
                                            lerp_rgba(theme.text_primary, on_accent, weight);
                                        let date_color = lerp_rgba(
                                            theme.text_muted,
                                            with_alpha(on_accent, 0.8),
                                            weight,
                                        );
                                        let file_color = lerp_rgba(
                                            theme.text_secondary,
                                            with_alpha(on_accent, 0.85),
                                            weight,
                                        );
                                        let tile_fill = lerp_rgba(
                                            theme.button_fill,
                                            lighting::shade(theme.button_fill, 0.22),
                                            weight,
                                        );
                                        let name = if project.name.is_empty() {
                                            format!("Project {}", index + 1)
                                        } else {
                                            project.name.clone()
                                        };
                                        let kind = if project.kind.is_empty() {
                                            "Project".to_string()
                                        } else {
                                            project.kind.clone()
                                        };
                                        let monogram = kind_monogram(&kind);
                                        let icon = this.cached_file_icon(&project.file);
                                        let mut file_line = project.file.clone();
                                        if let Some(hint) =
                                            location_hint(&project.path, &folder_root, &name)
                                        {
                                            file_line = format!("{file_line}  \u{00b7}  {hint}");
                                        }
                                        let opened = if project.last_opened.is_empty() {
                                            String::new()
                                        } else {
                                            short_date(&project.last_opened)
                                        };
                                        div()
                                            .h(px(PROJECT_ROW_PITCH))
                                            .w_full()
                                            .child(
                                                div()
                                                    .id(ElementId::NamedInteger(
                                                        "project-row".into(),
                                                        index as u64,
                                                    ))
                                                    .size_full()
                                                    .relative()
                                                    .rounded(px(CONTROL_RADIUS))
                                                    .when(weight > 0.0, |el| {
                                                        el.bg(lighting::lit(row_bg, 0.08 * weight))
                                                            .shadow(lighting::faded(
                                                                lighting::raised(theme.is_dark_mode),
                                                                weight,
                                                            ))
                                                    })
                                                    .when(weight <= 0.0, |el| {
                                                        el.hover(move |style| style.bg(hover_bg))
                                                    })
                                                    .px(px(10.0))
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
                                                    // The creative app's document icon when the OS
                                                    // has one; otherwise a monogram tile.
                                                    .child(match icon {
                                                        Some(icon) => div()
                                                            .w(px(34.0))
                                                            .h(px(34.0))
                                                            .flex_none()
                                                            .child(
                                                                img(icon)
                                                                    .w(px(34.0))
                                                                    .h(px(34.0))
                                                                    .object_fit(ObjectFit::Contain),
                                                            )
                                                            .into_any_element(),
                                                        None => div()
                                                            .w(px(34.0))
                                                            .h(px(34.0))
                                                            .flex_none()
                                                            .rounded(px(8.0))
                                                            .bg(lighting::lit(tile_fill, 0.14))
                                                            .shadow(lighting::raised(theme.is_dark_mode))
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .font_weight(FontWeight::SEMIBOLD)
                                                                    .text_color(on_accent)
                                                                    .child(SharedString::from(monogram)),
                                                            )
                                                            .into_any_element(),
                                                    })
                                                    // Name with the last-opened date, then the
                                                    // file (and location, when it adds anything).
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .flex()
                                                            .flex_col()
                                                            .gap(px(2.0))
                                                            .child(
                                                                div()
                                                                    .flex()
                                                                    .items_baseline()
                                                                    .gap(px(8.0))
                                                                    .child(
                                                                        div()
                                                                            .flex_1()
                                                                            .min_w_0()
                                                                            .text_size(px(13.5))
                                                                            .font_weight(FontWeight::SEMIBOLD)
                                                                            .text_color(name_color)
                                                                            .truncate()
                                                                            .child(SharedString::from(name)),
                                                                    )
                                                                    .when(!opened.is_empty(), |el| {
                                                                        el.child(
                                                                            div()
                                                                                .flex_none()
                                                                                .text_size(px(11.0))
                                                                                .text_color(date_color)
                                                                                .child(SharedString::from(opened)),
                                                                        )
                                                                    }),
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(12.0))
                                                                    .text_color(file_color)
                                                                    .truncate()
                                                                    .child(SharedString::from(file_line)),
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
                        self,
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
                                .text_size(px(13.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(theme.text_muted)
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
                self,
                cx,
            ))
    }
}
