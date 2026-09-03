//! Version graph pane (dotted grid, orthogonal links, ancestry highlight)
//! and the bottom details panel (blame, description, notes); both live in
//! the main view.

use gpui::{
    Context, ElementId, FontWeight, MouseButton, MouseDownEvent, Rgba, SharedString, Window,
    canvas, div, fill, point, prelude::*, px, size,
};

use crate::app::{ComboId, RootView, SelectionFade};
use crate::theme::{MONO_FONT, Theme};
use crate::ui::controls::{
    ButtonVariant, CONTROL_HEIGHT, ScrollAxis, caption, lerp_rgba, panel_button, scrollbar,
    text_area,
};
use crate::ui::graph_layout::{self, GraphNode, NODE_HALF_H, NODE_HALF_W};
use crate::ui::lighting;
use crate::ui::modals::delete_version_confirm;

impl RootView {
    pub(crate) fn render_graph_panel(&mut self, cx: &mut Context<RootView>) -> impl IntoElement {
        // The pane may have been resized since the last layout.
        self.sync_graph_columns();
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
            .shadow(lighting::panel(theme.is_dark_mode))
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

                                                    // Dotted grid, painted only for the
                                                    // visible clip — the full extent can be
                                                    // thousands of dots while the viewport
                                                    // shows ~700.
                                                    let grid_step =
                                                        (graph_layout::GRID * zoom).max(10.0);
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
                                                            (if highlighted { 2.5 } else { 2.0 })
                                                                * zoom.clamp(0.7, 1.5);
                                                        let color = if highlighted {
                                                            theme.vm_graph_link_active
                                                        } else {
                                                            theme.vm_graph_link
                                                        };
                                                        // Three axis-aligned quads rather than a
                                                        // stroked path: crisp edges with no
                                                        // jaggies. Positions snap to half pixels
                                                        // and the segments overlap by half the
                                                        // width so the corners close cleanly.
                                                        let snap = |v: f32| (v * 2.0).round() / 2.0;
                                                        let half = stroke_width * 0.5;
                                                        let mut segment =
                                                            |x: f32, y: f32, w: f32, h: f32| {
                                                                window.paint_quad(fill(
                                                                    gpui::Bounds::new(
                                                                        point(
                                                                            origin.x + px(snap(x)),
                                                                            origin.y + px(snap(y)),
                                                                        ),
                                                                        size(
                                                                            px(snap(w)),
                                                                            px(snap(h)),
                                                                        ),
                                                                    ),
                                                                    color,
                                                                ));
                                                            };
                                                        segment(
                                                            x1,
                                                            y1 - half,
                                                            mid_x - x1 + half,
                                                            stroke_width,
                                                        );
                                                        segment(
                                                            mid_x - half,
                                                            y1.min(y2) - half,
                                                            stroke_width,
                                                            (y2 - y1).abs() + stroke_width,
                                                        );
                                                        segment(
                                                            mid_x - half,
                                                            y2 - half,
                                                            x2 - mid_x + half,
                                                            stroke_width,
                                                        );
                                                    }
                                                },
                                            )
                                            .absolute()
                                            .size_full(),
                                        )
                                        // Nodes.
                                        .children(self.vm_graph.iter().enumerate().map(
                                            |(index, node)| {
                                                self.render_graph_node(index, node, zoom, cx)
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
                                        .rounded(px(4.0))
                                        .flex()
                                        .items_center()
                                        .cursor_pointer()
                                        .bg(lighting::lit(theme.button_soft_fill, 0.08))
                                        .border_1()
                                        .border_color(lighting::rim(
                                            theme.button_soft_fill,
                                            theme.is_dark_mode,
                                        ))
                                        .shadow(lighting::raised(theme.is_dark_mode))
                                        .text_size(px(10.0))
                                        .font_family(MONO_FONT)
                                        .text_color(theme.text_muted)
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
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(lighting::lit(theme.button_soft_fill, 0.08))
            .border_1()
            .border_color(lighting::rim(theme.button_soft_fill, theme.is_dark_mode))
            .shadow(lighting::raised(theme.is_dark_mode))
            .text_size(px(12.0))
            .text_color(theme.text_muted)
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
        node: &GraphNode,
        zoom: f32,
        cx: &mut Context<RootView>,
    ) -> impl IntoElement + use<> {
        let theme = self.theme;
        let selected = self.vm_selected_id == node.version.id;
        let current = node.version.is_current;
        let compressed = node.version.is_compressed;
        // 0 = plain node, 1 = fully selected; in between while the selection
        // cross-fades from one version to another.
        let weight = self
            .node_fade
            .as_ref()
            .map(|fade| fade.weight(&node.version.id, selected))
            .unwrap_or(if selected { 1.0 } else { 0.0 });
        // Text shrinks slower than geometry so labels stay legible when
        // zoomed out.
        let text_zoom = zoom.clamp(0.75, 1.6);

        let white: Rgba = gpui::white().into();
        let light = Rgba {
            r: 0xee as f32 / 255.0,
            g: 0xf2 as f32 / 255.0,
            b: 1.0,
            a: 1.0,
        };
        let fill_color = if current {
            theme.vm_node_current_fill
        } else {
            lerp_rgba(theme.vm_node_fill, theme.vm_node_selected_fill, weight)
        };
        let border_color = if current {
            theme.success_border
        } else {
            lerp_rgba(theme.node_border, theme.vm_node_selected_border, weight)
        };
        let label_color = if current {
            white
        } else {
            lerp_rgba(theme.vm_node_label, white, weight)
        };
        let sub_label_color = if current {
            light
        } else {
            lerp_rgba(theme.vm_text_meta, light, weight)
        };
        let border_width = if compressed { 1.5 } else { 1.0 } + weight;

        let mut node_shadows = lighting::raised(theme.is_dark_mode);
        if current {
            node_shadows.push(lighting::glow(theme.success_strong, 0.45, 8.0 * zoom));
        } else if weight > 0.0 {
            node_shadows.push(lighting::glow(theme.accent, 0.4 * weight, 8.0 * zoom));
        }
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
            // Body: lit card; the current and selected versions also glow.
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(8.0 * zoom))
                    .bg(lighting::lit(
                        fill_color,
                        if current { 0.1 } else { 0.05 + 0.05 * weight },
                    ))
                    .shadow(node_shadows)
                    .border(px(border_width))
                    // Dashes mark versions compaction has reduced to
                    // compressed objects; restoring one decompresses it.
                    .when(compressed, |el| el.border_dashed())
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
                            .font_family(MONO_FONT)
                            .font_weight(FontWeight::SEMIBOLD)
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
                                .font_family(MONO_FONT)
                                .text_color(sub_label_color)
                                .opacity(0.9)
                                .child(SharedString::from(node.version.full_label.clone())),
                        )
                    }),
            )
            // Click selects; double-click restores, like "Open Version".
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                    if event.click_count >= 2 {
                        this.pending_select_latest = true;
                        this.backend.restore_version_by_id(&version_id);
                    } else {
                        let previous =
                            std::mem::replace(&mut this.vm_selected_id, version_id.clone());
                        if previous != version_id {
                            this.node_fade = Some(SelectionFade {
                                to: version_id.clone(),
                                from: previous,
                                since: std::time::Instant::now(),
                            });
                        }
                        let note = note.clone();
                        this.version_note_input.update(cx, |input, cx| {
                            input.set_text(&note, cx);
                            input.disabled = false;
                        });
                    }
                    cx.notify();
                }),
            )
    }

    /// Bottom details panel: project info, project note, and the selected
    /// version's metadata, note and actions, as three columns split by
    /// hairlines.
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

        let version_label = match &selected_node {
            Some(node) if !node.version.full_label.is_empty() => node.version.full_label.clone(),
            Some(node) => node.version.id.clone(),
            None if has_selection => self.vm_selected_id.clone(),
            None => "\u{2014}".to_string(),
        };
        let selected_is_current = selected_node
            .as_ref()
            .is_some_and(|node| node.version.is_current);
        let saved_at = selected_node
            .as_ref()
            .map(|node| node.version.timestamp.clone())
            .filter(|timestamp| !timestamp.is_empty());
        let current_note = match (&current_node, selected_is_current) {
            (_, true) => None,
            (Some(node), false) => Some(format!("Current: {}", node.version.full_label)),
            (None, false) => Some("No saved version matches the file on disk".to_string()),
        };

        let panel = div()
            .size_full()
            .rounded(px(8.0))
            .bg(lighting::lit(theme.vm_side_panel, 0.02))
            .border_1()
            .border_color(theme.vm_border)
            .shadow(lighting::panel(theme.is_dark_mode))
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
                    .text_size(px(13.0))
                    .text_color(theme.vm_text_meta)
                    .child("Select a project to see its details and versions."),
            );
        };

        let home = std::env::var("HOME").ok().filter(|home| !home.is_empty());
        let path_display = match &home {
            Some(home) if project_item.path.starts_with(home.as_str()) => {
                format!("~{}", &project_item.path[home.len()..])
            }
            _ => project_item.path.clone(),
        };
        let last_opened = if project_item.last_opened.is_empty() {
            "never".to_string()
        } else {
            project_item.last_opened.clone()
        };

        let divider = || div().w(px(1.0)).h_full().flex_none().bg(theme.vm_border);

        // No inner scrolling: the note areas flex-fill whatever height the
        // divider gives the panel, so the action buttons stay visible and
        // the three columns keep a level top and bottom line.
        panel.child(
            div().size_full().p(px(14.0)).child(
                div()
                    .size_full()
                    .flex()
                    .gap(px(16.0))
                    // Column 1: project.
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            // Fixed-height blocks: the column can run short of
                            // room, and a shrinking text block clips to nothing.
                            .child(caption(&theme, "Project"))
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(theme.vm_text_primary)
                                    .truncate()
                                    .child(SharedString::from(project_item.name.clone())),
                            )
                            // Only what the list does not already show: the
                            // file (a picker when the project has several),
                            // where it lives, and when it was last opened.
                            .child(
                                div()
                                    .flex_none()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .map(|el| {
                                        if files.len() > 1 {
                                            el.child(
                                                div()
                                                    .h(px(CONTROL_HEIGHT))
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.0))
                                                    .child(detail_key(&theme, "File"))
                                                    .child(div().flex_1().min_w_0().child(
                                                        self.render_combo(
                                                            ComboId::PrimaryFile,
                                                            "primary-file",
                                                            primary_index,
                                                            &files,
                                                            None,
                                                            cx,
                                                            move |this, index, _w, cx| {
                                                                let files = this
                                                                    .backend
                                                                    .selected_project_files();
                                                                if let Some(file) = files.get(index)
                                                                {
                                                                    let file = file.clone();
                                                                    this.backend
                                                                        .set_selected_project_primary_file(
                                                                            &file,
                                                                        );
                                                                }
                                                                cx.notify();
                                                            },
                                                        ),
                                                    )),
                                            )
                                        } else {
                                            el.child(detail_row(
                                                &theme,
                                                "File",
                                                project_item.file.clone(),
                                            ))
                                        }
                                    })
                                    .child(detail_row(&theme, "Path", path_display))
                                    .child(detail_row(&theme, "Opened", last_opened)),
                            ),
                    )
                    .child(divider())
                    // Column 2: project note.
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .h(px(15.0))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .child(caption(&theme, "Project note")),
                            )
                            .child(text_area(
                                &self.project_note_input,
                                None,
                                true,
                                &theme,
                                window,
                                cx,
                            ))
                            .child(div().flex_none().flex().justify_end().child(
                                div().w(px(96.0)).child(panel_button(
                                    "save-project-note",
                                    "Save note",
                                    ButtonVariant::Soft,
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
                            )),
                    )
                    .child(divider())
                    // Column 3: selected version.
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .h(px(15.0))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .gap(px(7.0))
                                    .child(caption(&theme, "Version"))
                                    .child(
                                        div()
                                            .text_size(px(13.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(theme.vm_text_primary)
                                            .child(SharedString::from(version_label)),
                                    )
                                    // Right-aligned metas: save time, current state.
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .flex()
                                            .justify_end()
                                            .items_center()
                                            .gap(px(10.0))
                                            .when_some(saved_at, |el, saved_at| {
                                                el.child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap(px(4.0))
                                                        .text_size(px(11.0))
                                                        .text_color(theme.vm_text_meta)
                                                        .child("Saved")
                                                        .child(SharedString::from(saved_at)),
                                                )
                                            })
                                            .when(selected_is_current, |el| {
                                                el.child(
                                                    div().flex().items_center().gap(px(5.0)).child(
                                                        div()
                                                            .text_size(px(11.0))
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(theme.success_strong)
                                                            .child("Current"),
                                                    ),
                                                )
                                            })
                                            .when_some(current_note, |el, note| {
                                                el.child(
                                                    div()
                                                        .text_size(px(11.0))
                                                        .text_color(theme.vm_text_meta)
                                                        .truncate()
                                                        .child(SharedString::from(note)),
                                                )
                                            }),
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
                            // Actions: destructive on the left, the call to
                            // action on the right.
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .flex_none()
                                    .gap(px(8.0))
                                    .child(div().w(px(76.0)).child(panel_button(
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
                                            this.confirm = Some(delete_version_confirm(
                                                &this.vm_selected_id,
                                                this.show_dev_details(),
                                            ));
                                            cx.notify();
                                        },
                                    )))
                                    .child(div().flex_1())
                                    .child(div().w(px(100.0)).child(panel_button(
                                        "save-version-note",
                                        "Save note",
                                        ButtonVariant::Soft,
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
                                    .child(div().w(px(118.0)).child(panel_button(
                                        "open-version",
                                        "Open version",
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
                                    ))),
                            ),
                    ),
            ),
        )
    }
}

/// One line of the project inspector: a muted key in a fixed column and
/// its value.
fn detail_row(theme: &Theme, key: &str, value: String) -> impl IntoElement {
    div()
        // Same height as the file picker, so the rows share one rhythm.
        .h(px(CONTROL_HEIGHT))
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(detail_key(theme, key))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(12.0))
                .text_color(theme.vm_text_primary)
                .truncate()
                .child(SharedString::from(value)),
        )
}

/// Key column of the inspector rows.
fn detail_key(theme: &Theme, key: &str) -> impl IntoElement {
    div()
        .w(px(52.0))
        .flex_none()
        .text_size(px(12.0))
        .text_color(theme.vm_text_meta)
        .child(SharedString::from(key.to_string()))
}
