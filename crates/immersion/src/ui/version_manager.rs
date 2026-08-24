//! VersionManagerWindow.qml + VersionGraphNode.qml: version graph canvas
//! (dotted grid, orthogonal links, ancestry highlight) and the side panel.

use gpui::{
    AnyElement, Context, ElementId, FontWeight, MouseButton, PathBuilder, SharedString, Window,
    canvas, div, fill, point, prelude::*, px, size,
};

use crate::app::{ComboId, RootView};
use crate::theme::MODAL_PANEL_RADIUS;
use crate::ui::controls::{ButtonVariant, panel_button, text_area};
use crate::ui::modals::delete_version_confirm;

const NODE_HALF_W: f32 = 38.0;
const NODE_HALF_H: f32 = 23.0;

impl RootView {
    pub fn render_version_manager(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let viewport = window.viewport_size();
        let available_w = (f32::from(viewport.width) - crate::theme::MODAL_EDGE_PADDING).max(0.0);
        let available_h = (f32::from(viewport.height) - crate::theme::MODAL_EDGE_PADDING).max(0.0);
        let size = (
            available_w.min(720.0_f32.max(1120.0_f32.min(900.0))),
            available_h.min(480.0_f32.max(700.0_f32.min(560.0))),
        );

        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .p(px(14.0))
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
                            .child("Version Manager"),
                    )
                    .child(div().flex_1())
                    .child(panel_button(
                        "vm-close",
                        "Close",
                        ButtonVariant::Soft,
                        true,
                        &theme,
                        self,
                        cx,
                        |this, _w, cx| {
                            this.request_close_version_manager(cx);
                        },
                    )),
            )
            // Body: graph + side panel.
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .gap(px(12.0))
                    .child(self.render_graph_panel(cx))
                    .child(self.render_side_panel(window, cx)),
            );

        self.popup_frame_vm(size, content.into_any_element())
    }

    fn popup_frame_vm(&self, size: (f32, f32), content: AnyElement) -> impl IntoElement {
        use crate::ui::controls::modal_opacity;

        let theme = self.theme;
        let (opacity, _) = modal_opacity(self.vm_enter_at, self.vm_exit_at);
        div()
            .id("vm-popup-host")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("vm-popup")
                    .w(px(size.0))
                    .h(px(size.1))
                    .rounded(px(MODAL_PANEL_RADIUS))
                    .bg(theme.vm_panel)
                    .border_1()
                    .border_color(theme.modal_border)
                    .occlude()
                    .overflow_hidden()
                    .opacity(opacity)
                    .child(content),
            )
    }

    fn render_graph_panel(&mut self, cx: &mut Context<RootView>) -> impl IntoElement {
        let theme = self.theme;
        let graph_empty = self.vm_graph.is_empty();
        let (extent_w, extent_h) = self.vm_graph_extent;

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
                el.child(
                    div()
                        .id("vm-graph-scroll")
                        .absolute()
                        .inset(px(8.0))
                        .overflow_scroll()
                        .track_scroll(&self.graph_scroll)
                        .child(
                            div()
                                .w(px(extent_w))
                                .h(px(extent_h))
                                .relative()
                                // Canvas: dotted grid + links.
                                .child(
                                    canvas(
                                        |_bounds, _window, _cx| {},
                                        move |bounds, _state, window, _cx| {
                                            let origin = bounds.origin;

                                            // Dotted grid (28px, 1.1 radius dots).
                                            let grid_step = 28.0_f32;
                                            let grid_color = theme.vm_graph_grid;
                                            let width = f32::from(bounds.size.width);
                                            let height = f32::from(bounds.size.height);
                                            let mut gx = grid_step;
                                            while gx < width {
                                                let mut gy = grid_step;
                                                while gy < height {
                                                    window.paint_quad(
                                                        fill(
                                                            gpui::Bounds::new(
                                                                point(
                                                                    origin.x + px(gx - 1.1),
                                                                    origin.y + px(gy - 1.1),
                                                                ),
                                                                size(px(2.2), px(2.2)),
                                                            ),
                                                            grid_color,
                                                        )
                                                        .corner_radii(px(1.1)),
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
                                                let Some((px_, py_)) = positions.get(parent_id)
                                                else {
                                                    continue;
                                                };
                                                let highlighted = ancestors.contains(parent_id)
                                                    && ancestors.contains(id);
                                                let x1 = px_ + NODE_HALF_W;
                                                let y1 = *py_;
                                                let x2 = cx_ - NODE_HALF_W;
                                                let y2 = *cy_;
                                                let mid_x = x1 + (x2 - x1) * 0.5;

                                                let stroke_width =
                                                    if highlighted { 2.5 } else { 1.75 };
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
                                        self.render_graph_node(index, node.clone(), cx)
                                    },
                                )),
                        ),
                )
            })
    }

    fn render_graph_node(
        &self,
        index: usize,
        node: musit_core::backend::VersionGraphNode,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
        let selected = self.vm_selected_id == node.version.id;
        let current = node.version.is_current;

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
        let show_full = !node.version.full_label.is_empty()
            && node.version.full_label != node.version.label;

        div()
            .id(ElementId::NamedInteger("vm-node".into(), index as u64))
            .absolute()
            .left(px(node.x - NODE_HALF_W))
            .top(px(node.y - NODE_HALF_H))
            .w(px(NODE_HALF_W * 2.0))
            .h(px(NODE_HALF_H * 2.0))
            .cursor_pointer()
            // Shadow.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .mt(px(2.0))
                    .rounded(px(12.0))
                    .bg(theme.vm_node_shadow)
                    .opacity(if selected { 0.35 } else { 0.22 }),
            )
            // Body.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(12.0))
                    .bg(fill_color)
                    .when(selected, |el| el.border_2())
                    .when(!selected, |el| el.border_1())
                    .border_color(border_color)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(1.0))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(label_color)
                            .child(SharedString::from(if node.version.label.is_empty() {
                                "?".to_string()
                            } else {
                                node.version.label.clone()
                            })),
                    )
                    .when(show_full, |el| {
                        el.child(
                            div()
                                .text_size(px(9.0))
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
                        .top(px(-20.0))
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .h(px(16.0))
                                .px(px(5.0))
                                .rounded(px(8.0))
                                .bg(theme.success_strong)
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .text_size(px(8.0))
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
                    this.version_note_input
                        .update(cx, |input, cx| input.set_text(&note, cx));
                    cx.notify();
                }),
            )
    }

    fn render_side_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement {
        let theme = self.theme;
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
            format!("Current in project folder: {}", node.version.full_label)
        } else {
            "Current in project folder: no matching snapshot yet".to_string()
        };
        let meta_text = if let Some(node) = &selected_node {
            let timestamp = if node.version.timestamp.is_empty() {
                "unknown".to_string()
            } else {
                node.version.timestamp.clone()
            };
            let note = if node.version.note.is_empty() {
                "<none>".to_string()
            } else {
                node.version.note.clone()
            };
            format!("Time: {timestamp}\nCurrent note: {note}")
        } else {
            "Select a blob in the graph to inspect metadata.".to_string()
        };

        div()
            .w(px(340.0))
            .h_full()
            .flex_none()
            .rounded(px(8.0))
            .bg(theme.vm_side_panel)
            .border_1()
            .border_color(theme.vm_border)
            .overflow_hidden()
            .child(
                div()
                    .id("vm-side-scroll")
                    .size_full()
                    .p(px(10.0))
                    .overflow_y_scroll()
                    .track_scroll(&self.vm_side_scroll)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.vm_text_primary)
                                    .child("Project Note"),
                            )
                            // Main project file picker (only for multi-file projects).
                            .when(files.len() > 1, |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(6.0))
                                        .child(
                                            div()
                                                .text_size(px(13.0))
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(theme.vm_text_primary)
                                                .child("Main project file"),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .line_height(px(16.0))
                                                .text_color(theme.vm_text_meta)
                                                .child("Choose which file Immersion opens and versions when several project files share this folder."),
                                        )
                                        .child(self.render_combo(
                                            ComboId::PrimaryFile,
                                            "primary-file",
                                            primary_index,
                                            &files,
                                            None,
                                            cx,
                                            move |this, index, _w, cx| {
                                                let files =
                                                    this.backend.selected_project_files();
                                                if let Some(file) = files.get(index) {
                                                    let file = file.clone();
                                                    this.backend
                                                        .set_selected_project_primary_file(&file);
                                                }
                                                cx.notify();
                                            },
                                        )),
                                )
                            })
                            .child(text_area(
                                &self.project_note_input,
                                110.0,
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
                            ))
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.vm_text_primary)
                                    .child(SharedString::from(version_heading)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme.success_strong)
                                    .child(SharedString::from(current_line)),
                            )
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .line_height(px(17.0))
                                    .text_color(theme.vm_text_meta)
                                    .child(SharedString::from(meta_text)),
                            )
                            .child(text_area(
                                &self.version_note_input,
                                120.0,
                                has_selection,
                                &theme,
                                window,
                                cx,
                            ))
                            .child(panel_button(
                                "save-version-note",
                                "Save Version Note",
                                ButtonVariant::Primary,
                                has_selection,
                                &theme,
                                self,
                                cx,
                                |this, _w, cx| {
                                    let note = this.version_note_input.read(cx).text();
                                    let version_id = this.vm_selected_id.clone();
                                    if this.backend.save_version_note(&version_id, &note) {
                                        this.refresh_version_graph(false, cx);
                                    }
                                    cx.notify();
                                },
                            ))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(8.0))
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
                                            if this.backend.restore_version_by_id(&version_id) {
                                                this.refresh_version_graph(true, cx);
                                                this.request_close_version_manager(cx);
                                            } else {
                                                cx.notify();
                                            }
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
                                        |this, _w, cx| {
                                            this.confirm_enter_at = Some(std::time::Instant::now());
                                            this.confirm_exit_at = None;
                                            this.confirm = Some(delete_version_confirm(
                                                &this.vm_selected_id,
                                            ));
                                            cx.notify();
                                        },
                                    ))),
                            ),
                    ),
            )
    }
}
