//! Version graph pane (VersionGraphNode.qml port: dotted grid, orthogonal
//! links, ancestry highlight) and the bottom details panel (blame,
//! description, notes) — both live in the main view now.

use gpui::{
    Context, ElementId, FontWeight, MouseButton, PathBuilder, SharedString, Window, canvas, div,
    fill, point, prelude::*, px, size,
};

use crate::app::{ComboId, RootView};
use crate::ui::controls::{ButtonVariant, ScrollAxis, panel_button, scrollbar, text_area};
use crate::ui::modals::delete_version_confirm;

const NODE_HALF_W: f32 = 38.0;
const NODE_HALF_H: f32 = 23.0;

impl RootView {
    pub(crate) fn render_graph_panel(&mut self, cx: &mut Context<RootView>) -> impl IntoElement {
        let theme = self.theme;
        let graph_empty = self.vm_graph.is_empty();
        let (extent_w, extent_h) = self.vm_graph_extent;
        let zoom = self.graph_zoom;
        // The content is at least viewport-sized (min_w_full/min_h_full below)
        // so the dotted grid covers the whole pane even when the graph is
        // small — taffy resolves those against this frame's container, unlike
        // a max() against the previous frame's `graph_scroll.bounds()`.
        let content_w = extent_w * zoom;
        let content_h = extent_h * zoom;

        // Data for the paint closure.
        let nodes: Vec<(String, String, f32, f32)> = self
            .vm_graph
            .iter()
            .map(|node| {
                (
                    node.version.id.clone(),
                    node.parent_id.clone(),
                    node.x,
                    node.y,
                )
            })
            .collect();
        let positions: std::collections::HashMap<String, (f32, f32)> = nodes
            .iter()
            .map(|(id, _, x, y)| (id.clone(), (*x, *y)))
            .collect();
        let ancestors = self.ancestor_id_set(&self.vm_selected_id);

        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .rounded(px(8.0))
            .bg(theme.vm_graph)
            .border_1()
            .border_color(theme.vm_border)
            .overflow_hidden()
            .relative()
            .when(graph_empty, |el| {
                el.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(15.0))
                        .text_color(theme.vm_text_meta)
                        .child("No versions yet"),
                )
            })
            .when(!graph_empty, |el| {
                // Scrollbars live inside the same 8px-inset frame as the
                // scroll container so their geometry matches the viewport.
                el.child(
                    // NB: absolute only — a trailing `.relative()` would clobber
                    // the position and collapse the scroll viewport's height.
                    div()
                        .absolute()
                        .inset(px(8.0))
                        .child(
                            div()
                                .id("vm-graph-scroll")
                                .size_full()
                                .overflow_scroll()
                                // Lock a trackpad gesture to the axis it started on;
                                // without it gpui picks the dominant axis per event and
                                // a near-vertical swipe flaps between the two.
                                .restrict_scroll_to_axis()
                                .track_scroll(&self.graph_scroll)
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(content_w))
                                        .h(px(content_h))
                                        .min_w_full()
                                        .min_h_full()
                                        .relative()
                                        // Cmd/Ctrl + wheel zooms around the cursor. This
                                        // lives on the child, not the scroll container:
                                        // gpui registers its built-in pan handler after
                                        // ours and bubble dispatch runs in reverse, so a
                                        // listener on the container itself would pan
                                        // before it could stop propagation.
                                        .on_scroll_wheel(cx.listener(
                                            |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                                                if !(event.modifiers.platform
                                                    || event.modifiers.control)
                                                {
                                                    return;
                                                }
                                                let dy = match event.delta {
                                                    gpui::ScrollDelta::Pixels(delta) => {
                                                        f32::from(delta.y)
                                                    }
                                                    gpui::ScrollDelta::Lines(delta) => {
                                                        delta.y * 20.0
                                                    }
                                                };
                                                let factor = (1.0 + dy * 0.01).clamp(0.5, 2.0);
                                                this.set_graph_zoom(
                                                    this.graph_zoom * factor,
                                                    Some(event.position),
                                                    cx,
                                                );
                                                cx.stop_propagation();
                                            },
                                        ))
                                        // Canvas: dotted grid + links.
                                        .child(
                                            canvas(
                                                |_bounds, _window, _cx| {},
                                                move |bounds, _state, window, _cx| {
                                                    let origin = bounds.origin;

                                                    // Dotted grid (28px at 100% zoom),
                                                    // painted only for the visible clip —
                                                    // the full extent can be thousands of
                                                    // dots while the viewport shows ~700.
                                                    let grid_step = (28.0_f32 * zoom).max(10.0);
                                                    let dot_radius = 1.1_f32 * zoom.clamp(0.7, 1.6);
                                                    let grid_color = theme.vm_graph_grid;
                                                    let width = f32::from(bounds.size.width);
                                                    let height = f32::from(bounds.size.height);
                                                    let mask = window.content_mask().bounds;
                                                    let visible_left = (f32::from(mask.left())
                                                        - f32::from(origin.x))
                                                    .max(0.0);
                                                    let visible_top = (f32::from(mask.top())
                                                        - f32::from(origin.y))
                                                    .max(0.0);
                                                    let visible_right = (f32::from(mask.right())
                                                        - f32::from(origin.x))
                                                    .min(width);
                                                    let visible_bottom = (f32::from(mask.bottom())
                                                        - f32::from(origin.y))
                                                    .min(height);
                                                    let mut gx =
                                                        (visible_left / grid_step).floor().max(1.0)
                                                            * grid_step;
                                                    while gx < visible_right {
                                                        let mut gy = (visible_top / grid_step)
                                                            .floor()
                                                            .max(1.0)
                                                            * grid_step;
                                                        while gy < visible_bottom {
                                                            window.paint_quad(
                                                                fill(
                                                                    gpui::Bounds::new(
                                                                        point(
                                                                            origin.x
                                                                                + px(
                                                                                    gx - dot_radius
                                                                                ),
                                                                            origin.y
                                                                                + px(
                                                                                    gy - dot_radius
                                                                                ),
                                                                        ),
                                                                        size(
                                                                            px(dot_radius * 2.0),
                                                                            px(dot_radius * 2.0),
                                                                        ),
                                                                    ),
                                                                    grid_color,
                                                                )
                                                                .corner_radii(px(dot_radius)),
                                                            );
                                                            gy += grid_step;
                                                        }
                                                        gx += grid_step;
                                                    }

                                                    // Orthogonal parent->child links.
                                                    for (id, parent_id, cx_, cy_) in &nodes {
                                                        if parent_id.is_empty() {
                                                            continue;
                                                        }
                                                        let Some((px_, py_)) =
                                                            positions.get(parent_id)
                                                        else {
                                                            continue;
                                                        };
                                                        let highlighted = ancestors
                                                            .contains(parent_id)
                                                            && ancestors.contains(id);
                                                        let x1 = (px_ + NODE_HALF_W) * zoom;
                                                        let y1 = *py_ * zoom;
                                                        let x2 = (cx_ - NODE_HALF_W) * zoom;
                                                        let y2 = *cy_ * zoom;
                                                        let mid_x = x1 + (x2 - x1) * 0.5;

                                                        let stroke_width =
                                                            (if highlighted { 2.5 } else { 1.75 })
                                                                * zoom.clamp(0.7, 1.5);
                                                        let color = if highlighted {
                                                            theme.vm_graph_link_active
                                                        } else {
                                                            theme.vm_graph_link
                                                        };
                                                        let mut builder =
                                                            PathBuilder::stroke(px(stroke_width));
                                                        builder.move_to(point(
                                                            origin.x + px(x1),
                                                            origin.y + px(y1),
                                                        ));
                                                        builder.line_to(point(
                                                            origin.x + px(mid_x),
                                                            origin.y + px(y1),
                                                        ));
                                                        builder.line_to(point(
                                                            origin.x + px(mid_x),
                                                            origin.y + px(y2),
                                                        ));
                                                        builder.line_to(point(
                                                            origin.x + px(x2),
                                                            origin.y + px(y2),
                                                        ));
                                                        if let Ok(path) = builder.build() {
                                                            window.paint_path(path, color);
                                                        }
                                                    }
                                                },
                                            )
                                            .absolute()
                                            .size_full(),
                                        )
                                        // Nodes.
                                        .children(self.vm_graph.iter().enumerate().map(
                                            |(index, node)| {
                                                self.render_graph_node(
                                                    index,
                                                    node.clone(),
                                                    zoom,
                                                    cx,
                                                )
                                            },
                                        )),
                                ),
                        )
                        .child(scrollbar(
                            "graph-v",
                            &self.graph_scroll.clone(),
                            ScrollAxis::Vertical,
                            &theme,
                            cx,
                        ))
                        .child(scrollbar(
                            "graph-h",
                            &self.graph_scroll.clone(),
                            ScrollAxis::Horizontal,
                            &theme,
                            cx,
                        ))
                        // Zoom controls (also: cmd/ctrl + wheel).
                        .child(
                            div()
                                .absolute()
                                .top(px(6.0))
                                .right(px(14.0))
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(self.graph_zoom_button(
                                    "graph-zoom-out",
                                    "\u{2212}",
                                    cx,
                                    |this, cx| {
                                        this.set_graph_zoom(this.graph_zoom / 1.15, None, cx);
                                    },
                                ))
                                .child(
                                    div()
                                        .id("graph-zoom-reset")
                                        .h(px(22.0))
                                        .px(px(6.0))
                                        .rounded(px(5.0))
                                        .flex()
                                        .items_center()
                                        .cursor_pointer()
                                        .bg(theme.panel_surface)
                                        .border_1()
                                        .border_color(theme.node_border)
                                        .text_size(px(10.0))
                                        .text_color(theme.text_secondary)
                                        .hover(move |style| style.bg(theme.button_soft_fill))
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|this, _event, _window, cx| {
                                                this.set_graph_zoom(1.0, None, cx);
                                            }),
                                        )
                                        .child(SharedString::from(format!("{:.0}%", zoom * 100.0))),
                                )
                                .child(self.graph_zoom_button(
                                    "graph-zoom-in",
                                    "+",
                                    cx,
                                    |this, cx| {
                                        this.set_graph_zoom(this.graph_zoom * 1.15, None, cx);
                                    },
                                )),
                        ),
                )
            })
    }

    fn graph_zoom_button(
        &self,
        id: &'static str,
        glyph: &'static str,
        cx: &mut Context<RootView>,
        on_click: impl Fn(&mut RootView, &mut Context<RootView>) + 'static,
    ) -> impl IntoElement {
        let theme = self.theme;
        div()
            .id(id)
            .w(px(22.0))
            .h(px(22.0))
            .rounded(px(5.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(theme.panel_surface)
            .border_1()
            .border_color(theme.node_border)
            .text_size(px(12.0))
            .text_color(theme.text_secondary)
            .hover(move |style| style.bg(theme.button_soft_fill))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _event, _window, cx| {
                    on_click(this, cx);
                }),
            )
            .child(glyph)
    }

    fn render_graph_node(
        &self,
        index: usize,
        node: musit_core::backend::VersionGraphNode,
        zoom: f32,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement + use<> {
        let theme = self.theme;
        let selected = self.vm_selected_id == node.version.id;
        let current = node.version.is_current;
        // Text shrinks slower than geometry so labels stay legible when
        // zoomed out.
        let text_zoom = zoom.clamp(0.75, 1.6);

        let fill_color = if current {
            theme.vm_node_current_fill
        } else if selected {
            theme.vm_node_selected_fill
        } else {
            theme.vm_node_fill
        };
        let border_color = if current {
            theme.success_border
        } else if selected {
            theme.vm_node_selected_border
        } else {
            theme.node_border
        };
        let label_color = if current || selected {
            gpui::white().into()
        } else {
            theme.vm_node_label
        };
        let sub_label_color: gpui::Rgba = if current || selected {
            gpui::Rgba {
                r: 0xee as f32 / 255.0,
                g: 0xf2 as f32 / 255.0,
                b: 1.0,
                a: 1.0,
            }
        } else {
            theme.vm_text_meta
        };

        let version_id = node.version.id.clone();
        let note = node.version.note.clone();
        let show_full =
            !node.version.full_label.is_empty() && node.version.full_label != node.version.label;

        div()
            .id(ElementId::NamedInteger("vm-node".into(), index as u64))
            .absolute()
            .left(px((node.x - NODE_HALF_W) * zoom))
            .top(px((node.y - NODE_HALF_H) * zoom))
            .w(px(NODE_HALF_W * 2.0 * zoom))
            .h(px(NODE_HALF_H * 2.0 * zoom))
            .cursor_pointer()
            // Shadow.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .mt(px(2.0 * zoom))
                    .rounded(px(12.0 * zoom))
                    .bg(theme.vm_node_shadow)
                    .opacity(if selected { 0.35 } else { 0.22 }),
            )
            // Body.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(12.0 * zoom))
                    .bg(fill_color)
                    .when(selected, |el| el.border_2())
                    .when(!selected, |el| el.border_1())
                    .border_color(border_color)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(1.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .text_size(px(12.0 * text_zoom))
                            .font_weight(FontWeight::BOLD)
                            .text_color(label_color)
                            .child(SharedString::from(if node.version.label.is_empty() {
                                "?".to_string()
                            } else {
                                node.version.label.clone()
                            })),
                    )
                    .when(show_full && zoom > 0.7, |el| {
                        el.child(
                            div()
                                .text_size(px(9.0 * text_zoom))
                                .text_color(sub_label_color)
                                .opacity(0.92)
                                .child(SharedString::from(node.version.full_label.clone())),
                        )
                    }),
            )
            // CURRENT badge above the node.
            .when(current, |el| {
                el.child(
                    div()
                        .absolute()
                        .top(px(-20.0 * zoom))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .h(px(16.0 * text_zoom))
                                .px(px(5.0 * text_zoom))
                                .rounded(px(8.0 * text_zoom))
                                .bg(theme.success_strong)
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .text_size(px(8.0 * text_zoom))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(gpui::white())
                                        .child("CURRENT"),
                                ),
                        ),
                )
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _event, _window, cx| {
                    this.vm_selected_id = version_id.clone();
                    let note = note.clone();
                    this.version_note_input.update(cx, |input, cx| {
                        input.set_text(&note, cx);
                        input.disabled = false;
                    });
                    cx.notify();
                }),
            )
    }

    /// Bottom details panel: project info ("blame"), project note, and the
    /// selected version's metadata/note/actions, side by side.
    pub(crate) fn render_details_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let selected_index = self.backend.selected_project_index();
        let project_item = if selected_index >= 0 {
            self.backend
                .projects()
                .get(selected_index as usize)
                .cloned()
        } else {
            None
        };
        let files = self.backend.selected_project_files();
        let current_file = self.backend.selected_project_primary_file().to_string();
        let primary_index = files
            .iter()
            .position(|file| *file == current_file)
            .unwrap_or(0);
        let has_selection = !self.vm_selected_id.is_empty();
        let selected_node = self.node_by_id(&self.vm_selected_id).cloned();
        let current_node = self.current_version_node().cloned();
        let selected_id = self.vm_selected_id.clone();

        let version_heading = if let Some(node) = &selected_node {
            format!(
                "Version {}",
                if node.version.full_label.is_empty() {
                    node.version.id.clone()
                } else {
                    node.version.full_label.clone()
                }
            )
        } else if has_selection {
            format!("Version {selected_id}")
        } else {
            "No version selected".to_string()
        };
        let current_line = if let Some(node) = &current_node {
            format!("Current: {}", node.version.full_label)
        } else {
            "No saved version matches the file on disk".to_string()
        };
        let time_line = if let Some(node) = &selected_node {
            if node.version.timestamp.is_empty() {
                "Saved: unknown".to_string()
            } else {
                format!("Saved: {}", node.version.timestamp)
            }
        } else {
            "Select a version in the graph.".to_string()
        };

        let panel = div()
            .size_full()
            .rounded(px(8.0))
            .bg(theme.vm_side_panel)
            .border_1()
            .border_color(theme.vm_border)
            .overflow_hidden()
            .relative();

        // Empty state: nothing selected yet.
        let Some(project_item) = project_item else {
            return panel.child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(14.0))
                    .text_color(theme.vm_text_meta)
                    .child("Select a project to see its details and versions."),
            );
        };

        // No inner scrolling: the note areas flex-fill whatever height the
        // divider gives the panel, so the action buttons stay visible and
        // the three columns keep a level top and bottom line.
        panel.child(
            div().size_full().p(px(12.0)).child(
                div()
                    .size_full()
                    .flex()
                    .gap(px(14.0))
                    // Column 1: project info ("blame").
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.vm_text_primary)
                                    .child(SharedString::from(project_item.name.clone())),
                            )
                            .child(meta_line(&theme, format!("Kind: {}", project_item.kind)))
                            .child(meta_line(
                                &theme,
                                format!("Project file: {}", project_item.file),
                            ))
                            .child(meta_line(&theme, format!("Path: {}", project_item.path)))
                            .child(meta_line(
                                &theme,
                                if project_item.last_opened.is_empty() {
                                    "Last opened: never".to_string()
                                } else {
                                    format!("Last opened: {}", project_item.last_opened)
                                },
                            ))
                            // Main project file picker (multi-file projects).
                            .when(files.len() > 1, |el| {
                                el.child(
                                    div()
                                        .mt(px(4.0))
                                        .flex()
                                        .flex_col()
                                        .gap(px(4.0))
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme.vm_text_primary)
                                                .child("Main project file"),
                                        )
                                        .child(self.render_combo(
                                            ComboId::PrimaryFile,
                                            "primary-file",
                                            primary_index,
                                            &files,
                                            None,
                                            cx,
                                            move |this, index, _w, cx| {
                                                let files = this.backend.selected_project_files();
                                                if let Some(file) = files.get(index) {
                                                    let file = file.clone();
                                                    this.backend
                                                        .set_selected_project_primary_file(&file);
                                                }
                                                cx.notify();
                                            },
                                        )),
                                )
                            }),
                    )
                    // Column 2: project note.
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.vm_text_primary)
                                    .child("Project Note"),
                            )
                            .child(text_area(
                                &self.project_note_input,
                                None,
                                true,
                                &theme,
                                window,
                                cx,
                            ))
                            .child(panel_button(
                                "save-project-note",
                                "Save Project Note",
                                ButtonVariant::Primary,
                                true,
                                &theme,
                                self,
                                cx,
                                |this, _w, cx| {
                                    let note = this.project_note_input.read(cx).text();
                                    this.backend.set_selected_project_note(&note);
                                    cx.notify();
                                },
                            )),
                    )
                    // Column 3: selected version details, note, actions.
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(theme.vm_text_primary)
                                            .child(SharedString::from(version_heading)),
                                    )
                                    // Metas right-aligned beside the heading:
                                    // save time and the on-disk current marker.
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .justify_end()
                                            .items_center()
                                            .gap(px(8.0))
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .text_color(theme.vm_text_meta)
                                                    .truncate()
                                                    .child(SharedString::from(time_line)),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(theme.success_strong)
                                                    .truncate()
                                                    .child(SharedString::from(current_line)),
                                            ),
                                    ),
                            )
                            .child(text_area(
                                &self.version_note_input,
                                None,
                                has_selection,
                                &theme,
                                window,
                                cx,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.0))
                                    .child(div().flex_1().child(panel_button(
                                        "save-version-note",
                                        "Save Note",
                                        ButtonVariant::Primary,
                                        has_selection,
                                        &theme,
                                        self,
                                        cx,
                                        |this, _w, cx| {
                                            let note = this.version_note_input.read(cx).text();
                                            let version_id = this.vm_selected_id.clone();
                                            // The backend event refreshes the graph.
                                            this.backend.save_version_note(&version_id, &note);
                                            cx.notify();
                                        },
                                    )))
                                    .child(div().flex_1().child(panel_button(
                                        "open-version",
                                        "Open Version",
                                        ButtonVariant::Primary,
                                        has_selection,
                                        &theme,
                                        self,
                                        cx,
                                        |this, _w, cx| {
                                            let version_id = this.vm_selected_id.clone();
                                            this.pending_select_latest = true;
                                            this.backend.restore_version_by_id(&version_id);
                                            cx.notify();
                                        },
                                    )))
                                    .child(div().flex_1().child(panel_button(
                                        "delete-version",
                                        "Delete",
                                        ButtonVariant::Danger,
                                        has_selection,
                                        &theme,
                                        self,
                                        cx,
                                        |this, w, cx| {
                                            this.take_modal_focus(w, cx);
                                            this.confirm_enter_at = Some(std::time::Instant::now());
                                            this.confirm_exit_at = None;
                                            this.confirm =
                                                Some(delete_version_confirm(
                                                    &this.vm_selected_id,
                                                    this.show_dev_details(),
                                                ));
                                            cx.notify();
                                        },
                                    ))),
                            ),
                    ),
            ),
        )
    }
}

fn meta_line(theme: &crate::theme::Theme, text: String) -> impl IntoElement {
    div()
        .text_size(px(12.0))
        .line_height(px(16.0))
        .text_color(theme.vm_text_meta)
        .truncate()
        .child(SharedString::from(text))
}
