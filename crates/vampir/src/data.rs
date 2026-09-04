//! Rows of data: a tree and a sortable table header.
//!
//! Neither owns its data. A tree takes an already flattened list of visible
//! rows, and a header takes the current sort; both report clicks and leave
//! the model to the host. That is what lets either sit above a
//! `uniform_list` and stay cheap with thousands of rows.

use std::rc::Rc;

use gpui::{
    AnyElement, Context, ElementId, FontWeight, MouseButton, MouseDownEvent, PathBuilder,
    SharedString, Window, canvas, div, point, prelude::*, px,
};

use crate::lighting;
use crate::palette::Palette;
use crate::state::ControlHost;

// ---- Tree -------------------------------------------------------------------

/// One visible row of a tree.
///
/// The host flattens its own model into these each frame, skipping the
/// children of collapsed rows. Nothing here walks a tree, so the model can
/// be whatever shape the host already has.
#[derive(Clone, Debug)]
pub struct TreeRow {
    /// Comes back to the callbacks.
    pub id: SharedString,
    pub label: SharedString,
    /// How deep the row sits. Zero is a root.
    pub depth: usize,
    /// `None` for a leaf, which gets no chevron and no disclosure hit area.
    pub expanded: Option<bool>,
    /// Trailing text, for a count or a size.
    pub detail: Option<SharedString>,
}

impl TreeRow {
    pub fn leaf(id: impl Into<SharedString>, label: impl Into<SharedString>, depth: usize) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            depth,
            expanded: None,
            detail: None,
        }
    }

    pub fn branch(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        depth: usize,
        expanded: bool,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            depth,
            expanded: Some(expanded),
            detail: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// How far each level of nesting is indented.
pub const TREE_INDENT: f32 = 14.0;
/// Row height, matching a pop-up list's so a tree and a menu beside each
/// other line up.
pub const TREE_ROW_HEIGHT: f32 = 24.0;

/// Renders one tree row.
///
/// Kept per-row rather than per-tree so it can go straight into a
/// `uniform_list`, which is what keeps a large tree affordable.
///
/// The chevron and the row are separate hit targets: clicking the chevron
/// expands, clicking the row selects. A host that wants clicking anywhere to
/// expand can simply do that in `on_select`.
#[allow(clippy::too_many_arguments)]
pub fn tree_row<V: ControlHost>(
    id: &'static str,
    index: usize,
    row: &TreeRow,
    selected: bool,
    palette: Palette,
    cx: &mut Context<V>,
    on_select: impl Fn(&mut V, SharedString, &mut Window, &mut Context<V>) + 'static,
    on_toggle: impl Fn(&mut V, SharedString, bool, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let chevron: gpui::Hsla = palette.text_secondary.into();
    let row_id = row.id.clone();
    let toggle_id = row.id.clone();
    let expanded = row.expanded;

    div()
        .id(ElementId::NamedInteger(
            format!("{id}-row").into(),
            index as u64,
        ))
        .h(px(TREE_ROW_HEIGHT))
        .w_full()
        .flex_none()
        .pl(px(4.0 + row.depth as f32 * TREE_INDENT))
        .pr(px(8.0))
        .flex()
        .items_center()
        .gap(px(4.0))
        .rounded(px(4.0))
        .cursor_pointer()
        .text_size(px(12.5))
        .text_color(if selected {
            palette.control_label
        } else {
            palette.text_primary
        })
        .when(selected, |el| {
            el.bg(lighting::lit(palette.control_fill, 0.08))
        })
        .when(!selected, |el| {
            el.hover(move |style| style.bg(palette.row_hover))
        })
        .on_click(cx.listener(move |this, _event, window, cx| {
            on_select(this, row_id.clone(), window, cx);
            cx.notify();
        }))
        .child(
            // A leaf still reserves the chevron's width, so labels down a
            // level line up whether or not their siblings have children.
            div()
                .id(ElementId::NamedInteger(
                    format!("{id}-chevron").into(),
                    index as u64,
                ))
                .w(px(12.0))
                .h(px(12.0))
                .flex_none()
                .when_some(expanded, |el, open| {
                    el.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
                            on_toggle(this, toggle_id.clone(), !open, window, cx);
                            cx.notify();
                        }),
                    )
                    .child(
                        canvas(
                            |_bounds, _window, _cx| {},
                            move |bounds, _state, window, _cx| {
                                let o = bounds.origin;
                                let mut builder = PathBuilder::stroke(px(1.4));
                                if open {
                                    builder.move_to(point(o.x + px(2.0), o.y + px(4.5)));
                                    builder.line_to(point(o.x + px(6.0), o.y + px(8.5)));
                                    builder.line_to(point(o.x + px(10.0), o.y + px(4.5)));
                                } else {
                                    builder.move_to(point(o.x + px(4.5), o.y + px(2.0)));
                                    builder.line_to(point(o.x + px(8.5), o.y + px(6.0)));
                                    builder.line_to(point(o.x + px(4.5), o.y + px(10.0)));
                                }
                                if let Ok(path) = builder.build() {
                                    window.paint_path(path, chevron);
                                }
                            },
                        )
                        .size_full(),
                    )
                }),
        )
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .child(row.label.clone()),
        )
        .children(row.detail.clone().map(|detail| {
            div()
                .flex_none()
                .text_size(px(11.5))
                .text_color(palette.text_secondary)
                .child(detail)
        }))
}

// ---- Table header -----------------------------------------------------------

/// Which way a column is sorted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn flipped(self) -> Self {
        match self {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        }
    }
}

/// One column of a [`table_header`].
#[derive(Clone, Debug)]
pub struct Column {
    pub id: SharedString,
    pub label: SharedString,
    /// Fixed width in px, or `None` to share the leftover space evenly.
    pub width: Option<f32>,
    pub sortable: bool,
    /// Right-aligned, for numbers and sizes.
    pub numeric: bool,
}

impl Column {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            width: None,
            sortable: true,
            numeric: false,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn fixed(mut self) -> Self {
        self.sortable = false;
        self
    }

    /// Right-aligns the column, which is how a reader compares numbers.
    pub fn numeric(mut self) -> Self {
        self.numeric = true;
        self
    }
}

/// Header row for a table: column labels, with an arrow on the sorted one.
///
/// Clicking a sortable column reports it and the direction it should take:
/// the same column flips, a different one starts ascending, which is what
/// every table does and what nobody has to be told.
#[allow(clippy::too_many_arguments)]
pub fn table_header<V: ControlHost>(
    id: &'static str,
    columns: &[Column],
    sorted_by: Option<(&str, SortDirection)>,
    palette: Palette,
    cx: &mut Context<V>,
    on_sort: impl Fn(&mut V, SharedString, SortDirection, &mut Window, &mut Context<V>) + 'static,
) -> impl IntoElement {
    let on_sort = Rc::new(on_sort);
    let mut cells: Vec<AnyElement> = Vec::with_capacity(columns.len());

    for (index, column) in columns.iter().enumerate() {
        let cx: &mut Context<V> = &mut *cx;
        let on_sort = on_sort.clone();
        let column_id = column.id.clone();
        let active = sorted_by.filter(|(sorted, _)| *sorted == column.id.as_ref());
        let next = match active {
            Some((_, direction)) => direction.flipped(),
            None => SortDirection::Ascending,
        };
        let arrow_up = matches!(active, Some((_, SortDirection::Ascending)));
        let arrow: gpui::Hsla = palette.text_primary.into();
        cells.push(
            div()
                .id(ElementId::NamedInteger(
                    format!("{id}-column").into(),
                    index as u64,
                ))
                .h_full()
                .when_some(column.width, |el, width| el.w(px(width)).flex_none())
                .when(column.width.is_none(), |el| el.flex_1())
                .px(px(8.0))
                .flex()
                .items_center()
                .gap(px(4.0))
                .when(column.numeric, |el| el.justify_end())
                .text_size(px(11.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if active.is_some() {
                    palette.text_primary
                } else {
                    palette.text_secondary
                })
                .whitespace_nowrap()
                .overflow_hidden()
                .when(column.sortable, move |el| {
                    el.cursor_pointer()
                        .hover(move |style| style.bg(palette.row_hover))
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            on_sort(this, column_id.clone(), next, window, cx);
                            cx.notify();
                        }))
                })
                .child(column.label.clone())
                .when(active.is_some(), move |el| {
                    el.child(
                        div().w(px(9.0)).h(px(9.0)).flex_none().child(
                            canvas(
                                |_bounds, _window, _cx| {},
                                move |bounds, _state, window, _cx| {
                                    let o = bounds.origin;
                                    let mut builder = PathBuilder::stroke(px(1.3));
                                    if arrow_up {
                                        builder.move_to(point(o.x + px(0.5), o.y + px(6.5)));
                                        builder.line_to(point(o.x + px(4.5), o.y + px(2.5)));
                                        builder.line_to(point(o.x + px(8.5), o.y + px(6.5)));
                                    } else {
                                        builder.move_to(point(o.x + px(0.5), o.y + px(2.5)));
                                        builder.line_to(point(o.x + px(4.5), o.y + px(6.5)));
                                        builder.line_to(point(o.x + px(8.5), o.y + px(2.5)));
                                    }
                                    if let Ok(path) = builder.build() {
                                        window.paint_path(path, arrow);
                                    }
                                },
                            )
                            .size_full(),
                        ),
                    )
                })
                .into_any_element(),
        );
    }

    let mut rule: gpui::Hsla = palette.field_border.into();
    rule.a = 0.9;
    div()
        .id(id)
        .h(px(26.0))
        .w_full()
        .flex_none()
        .flex()
        .items_stretch()
        .border_b_1()
        .border_color(rule)
        .children(cells)
}
