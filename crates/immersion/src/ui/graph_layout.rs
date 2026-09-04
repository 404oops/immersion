//! Version-graph layout: places the version tree on the graph pane's dotted
//! grid.
//!
//! Top-level versions flow left to right and wrap onto a new line when the
//! pane runs out of columns, like words in a paragraph, so a long linear
//! history fills the pane instead of stacking into one tall column. A
//! version's branch subtree hangs off it to the right: children sit one
//! column over, the first on its parent's row and the rest stacked below
//! it, so a fan of sibling saves reads as a vertical list. The block a
//! top-level version occupies is therefore as wide as its deepest branch
//! and as tall as its largest fan; blocks sharing a line are top-aligned.
//!
//! Nodes are squares of 1.5 grid cells; one whose label would not fit is
//! widened in half-cell steps, and its column's pitch grows to match, so
//! nothing overlaps. The gap between nodes equals a node, in both
//! directions, so the graph reads as an even lattice. Every node centre
//! lands on a grid dot, and links descend through the gutter between two
//! columns.
//!
//! The wrap width is the pane width at the current zoom, in content units:
//! zooming out spreads the versions across the pane in more columns, and
//! zooming in narrows them into more rows. Whatever width is left over is
//! The columns keep one fixed pitch, so every node centre lands on the same
//! dot grid whatever the zoom; only how many columns fit changes. Spreading
//! the leftover width across the gaps instead would nudge the nodes against
//! the dots at each zoom step, which reads as the boxes drifting.
//!
//! A history too big for the pane keeps a roughly square shape: the cells
//! are square, so the column count is the square root of the version count,
//! never fewer than the pane fits. A big history therefore overflows in
//! both directions and reads as a block you drag around, rather than a
//! strip that only runs one way.

use std::collections::HashMap;

use musit_core::backend::{VersionEntry, VersionGraphNode};

/// Dot-grid pitch at 100% zoom.
pub const GRID: f32 = 28.0;
/// Node height, and the width of a node whose label fits.
pub const NODE_SIZE: f32 = 1.5 * GRID;
pub const NODE_HALF_H: f32 = NODE_SIZE / 2.0;
/// Pitch of a column of square nodes (node plus a node-sized gap); a wider
/// node widens its column.
pub const COL_PITCH: f32 = 3.0 * GRID;
pub const ROW_PITCH: f32 = 3.0 * GRID;
/// Font sizes of the node label and its smaller full-id line; the layout
/// sizes nodes for them, so the renderer uses the same values.
pub const LABEL_FONT_PX: f32 = 12.0;
pub const SUB_LABEL_FONT_PX: f32 = 9.0;

/// Centre of the first column and row: clearance beside the first node and
/// room above it.
const ORIGIN: f32 = 2.0 * GRID;
/// Space kept between the widest node of a column and the next column: a
/// node's width, matching the gap between rows.
const GUTTER: f32 = NODE_SIZE;
/// Horizontal padding inside a node around its label.
const LABEL_PAD: f32 = 8.0;
/// Clearance kept between the last column's right edge and the pane edge:
/// the lattice runs as far right as it can.
const RIGHT_EDGE: f32 = GRID / 2.0;
/// Advance width of the mono label font as a fraction of its size.
const MONO_ADVANCE: f32 = 0.62;

/// What a layout pass produced besides the node positions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Placement {
    /// Per column, the x of the gutter centre to its right, where links to
    /// the next column descend.
    pub gutters: Vec<f32>,
}

/// A fitted layout: the column count it settled on, plus its placement.
#[derive(Clone, Debug, PartialEq)]
pub struct Fitted {
    pub columns: usize,
    pub placement: Placement,
}

/// A version placed on the graph pane.
#[derive(Clone, Debug)]
pub struct GraphNode {
    pub version: VersionEntry,
    /// Parent version id; empty for a top-level version.
    pub parent_id: String,
    /// Centre, in content pixels at 100% zoom.
    pub x: f32,
    pub y: f32,
    /// Half the node's width: `NODE_HALF_H` for a square, more for a node
    /// whose label needs the room.
    pub half_w: f32,
    /// Grid cell the node occupies.
    pub col: usize,
    pub row: usize,
}

impl From<VersionGraphNode> for GraphNode {
    fn from(node: VersionGraphNode) -> Self {
        let half_w = node_half_width(&node.version.label, &node.version.full_label);
        Self {
            version: node.version,
            parent_id: node.parent_id,
            x: 0.0,
            y: 0.0,
            half_w,
            col: 0,
            row: 0,
        }
    }
}

/// Half the width a node needs for its text: a square unless the label or
/// the full-id line is longer, then the width grows in half-cell steps.
pub fn node_half_width(label: &str, full_label: &str) -> f32 {
    let mono_width =
        |text: &str, font_px: f32| text.chars().count() as f32 * font_px * MONO_ADVANCE;
    let label_width = mono_width(label, LABEL_FONT_PX);
    let sub_width = if full_label.is_empty() || full_label == label {
        0.0
    } else {
        mono_width(full_label, SUB_LABEL_FONT_PX)
    };
    let needed = label_width.max(sub_width) + 2.0 * LABEL_PAD;
    let step = GRID / 2.0;
    let width = ((needed / step).ceil() * step).max(NODE_SIZE);
    width / 2.0
}

/// Columns that fit a pane `viewport_width` wide (in content units, i.e. the
/// pane divided by the zoom), assuming square nodes: the first column, then
/// one more per pitch that still ends before the right-edge clearance.
/// `layout_fitting` corrects for widened columns.
pub fn columns_for_width(viewport_width: f32) -> usize {
    let first_right = ORIGIN + NODE_HALF_H;
    let usable = viewport_width - RIGHT_EDGE - first_right;
    if usable < 0.0 {
        1
    } else {
        (usable / COL_PITCH) as usize + 1
    }
}

/// Columns that make the lattice as square as it can be. The cells are
/// square, so that is the square root of the version count.
fn square_columns(node_count: usize) -> usize {
    (node_count as f32).sqrt().ceil().max(1.0) as usize
}

/// Lays the graph out for a pane `pane_width` wide (content units): it fills
/// the pane's width, and a history too big for that keeps a roughly square
/// shape instead of running off in one direction, so both scrolls stay
/// useful.
pub fn layout_for_pane(nodes: &mut [GraphNode], pane_width: f32) -> Fitted {
    let fitted = layout_fitting(nodes, pane_width);
    let square = square_columns(nodes.len());
    if square <= fitted.columns {
        return fitted;
    }
    let placement = layout(nodes, square);
    Fitted {
        columns: square,
        placement,
    }
}

/// Lays the graph out for a pane `viewport_width` wide (content units),
/// dropping a column at a time while widened columns push it past the pane,
/// then stretching the columns to span it.
pub fn layout_fitting(nodes: &mut [GraphNode], viewport_width: f32) -> Fitted {
    let mut columns = columns_for_width(viewport_width);
    loop {
        let placement = layout(nodes, columns);
        let (width, _) = extent(nodes);
        if columns <= 1 || width <= viewport_width + 0.5 {
            return Fitted { columns, placement };
        }
        columns -= 1;
    }
}

/// Assigns every node's cell and centre. `nodes` must list parents before
/// their children with siblings in display order (the order the backend
/// emits); that order is preserved. A node whose parent is missing, or
/// listed after it, is laid out as a root.
pub fn layout(nodes: &mut [GraphNode], columns: usize) -> Placement {
    let columns = columns.max(1);
    let index_by_id: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.version.id.as_str(), index))
        .collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let mut roots: Vec<usize> = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        match index_by_id.get(node.parent_id.as_str()) {
            Some(&parent) if parent < index => children[parent].push(index),
            _ => roots.push(index),
        }
    }

    // Block sizes, bottom-up: children always follow their parent, so a
    // reverse pass has sized every child before it reaches the parent.
    let mut width = vec![1usize; nodes.len()];
    let mut height = vec![1usize; nodes.len()];
    for index in (0..nodes.len()).rev() {
        if children[index].is_empty() {
            continue;
        }
        width[index] = 1 + children[index]
            .iter()
            .map(|&child| width[child])
            .max()
            .unwrap_or(0);
        height[index] = children[index].iter().map(|&child| height[child]).sum();
    }

    // Flow the top-level blocks; a block wider than the pane still starts a
    // line of its own and simply overflows to the right.
    let mut column = 0usize;
    let mut line_top = 0usize;
    let mut line_height = 0usize;
    for &root in &roots {
        if column > 0 && column + width[root] > columns {
            line_top += line_height;
            column = 0;
            line_height = 0;
        }
        place(nodes, &children, &height, root, column, line_top);
        column += width[root];
        line_height = line_height.max(height[root]);
    }

    // Column pitches follow the widest node in each column. Columns the
    // graph does not fill are square, so a partial row is spaced like a
    // full one.
    let used_columns = nodes.iter().map(|node| node.col + 1).max().unwrap_or(0);
    let column_count = columns.max(used_columns).max(1);
    let mut max_half = vec![NODE_HALF_H; column_count];
    for node in nodes.iter() {
        max_half[node.col] = max_half[node.col].max(node.half_w);
    }
    let pitches: Vec<f32> = max_half
        .iter()
        .map(|half| ((2.0 * half + GUTTER) / GRID).ceil() * GRID)
        .collect();
    let mut column_x = Vec::with_capacity(column_count);
    let mut x = ORIGIN;
    for pitch in &pitches {
        column_x.push(x);
        x += pitch;
    }
    for node in nodes.iter_mut() {
        node.x = column_x[node.col];
        node.y = ORIGIN + node.row as f32 * ROW_PITCH;
    }
    let gutters = (0..column_count)
        .map(|c| {
            let right = column_x[c] + max_half[c];
            let next_left = if c + 1 < column_count {
                column_x[c + 1] - max_half[c + 1]
            } else {
                right + GUTTER
            };
            (right + next_left) / 2.0
        })
        .collect();
    Placement { gutters }
}

fn place(
    nodes: &mut [GraphNode],
    children: &[Vec<usize>],
    height: &[usize],
    index: usize,
    column: usize,
    row: usize,
) {
    nodes[index].col = column;
    nodes[index].row = row;
    let mut child_row = row;
    for &child in &children[index] {
        place(nodes, children, height, child, column + 1, child_row);
        child_row += height[child];
    }
}

/// Scrollable content size: the far node edges plus the right-edge
/// clearance sideways (so a fitted layout never scrolls horizontally) and
/// the origin's clearance below.
pub fn extent(nodes: &[GraphNode]) -> (f32, f32) {
    let bottom_pad = ORIGIN - NODE_HALF_H;
    nodes
        .iter()
        .fold((0.0_f32, 0.0_f32), |(width, height), node| {
            (
                width.max(node.x + node.half_w + RIGHT_EDGE),
                height.max(node.y + NODE_HALF_H + bottom_pad),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rows the current cell assignment uses.
    fn rows_used(nodes: &[GraphNode]) -> usize {
        nodes.iter().map(|node| node.row + 1).max().unwrap_or(0)
    }

    fn node(id: &str, parent: &str) -> GraphNode {
        node_with_label(id, parent, id, "")
    }

    fn node_with_label(id: &str, parent: &str, label: &str, full_label: &str) -> GraphNode {
        GraphNode::from(VersionGraphNode {
            version: VersionEntry {
                id: id.to_string(),
                label: label.to_string(),
                full_label: full_label.to_string(),
                ..VersionEntry::default()
            },
            parent_id: parent.to_string(),
            depth: 0,
        })
    }

    fn cells(nodes: &[GraphNode]) -> Vec<(&str, (usize, usize))> {
        nodes
            .iter()
            .map(|node| (node.version.id.as_str(), (node.col, node.row)))
            .collect()
    }

    #[test]
    fn linear_history_wraps_into_rows() {
        let mut nodes: Vec<GraphNode> = (1..=7).map(|n| node(&n.to_string(), "")).collect();
        layout(&mut nodes, 3);
        assert_eq!(
            cells(&nodes),
            vec![
                ("1", (0, 0)),
                ("2", (1, 0)),
                ("3", (2, 0)),
                ("4", (0, 1)),
                ("5", (1, 1)),
                ("6", (2, 1)),
                ("7", (0, 2)),
            ]
        );
    }

    #[test]
    fn branches_hang_right_with_first_child_on_parent_row() {
        let mut nodes = vec![
            node("1", ""),
            node("1.1", "1"),
            node("1.2", "1"),
            node("2", ""),
            node("3", ""),
            node("4", ""),
        ];
        layout(&mut nodes, 3);
        assert_eq!(
            cells(&nodes),
            vec![
                ("1", (0, 0)),
                ("1.1", (1, 0)),
                ("1.2", (1, 1)),
                ("2", (2, 0)),
                ("3", (0, 2)),
                ("4", (1, 2)),
            ]
        );
    }

    #[test]
    fn block_too_wide_for_the_rest_of_the_line_wraps_whole() {
        let mut nodes = vec![
            node("1", ""),
            node("2", ""),
            node("2.1", "2"),
            node("2.1.2", "2.1"),
            node("2.1.3", "2.1"),
            node("2.2", "2"),
            node("3", ""),
            node("4", ""),
        ];
        layout(&mut nodes, 3);
        assert_eq!(
            cells(&nodes),
            vec![
                ("1", (0, 0)),
                ("2", (0, 1)),
                ("2.1", (1, 1)),
                ("2.1.2", (2, 1)),
                ("2.1.3", (2, 2)),
                ("2.2", (1, 3)),
                ("3", (0, 4)),
                ("4", (1, 4)),
            ]
        );
    }

    #[test]
    fn missing_parent_makes_a_root() {
        let mut nodes = vec![node("1", ""), node("5.1", "5")];
        layout(&mut nodes, 4);
        assert_eq!(cells(&nodes), vec![("1", (0, 0)), ("5.1", (1, 0))]);
    }

    #[test]
    fn short_labels_give_square_nodes_on_grid_dots() {
        let mut nodes = vec![
            node_with_label("1", "", "v1", "v1"),
            node_with_label("1.2", "1", ".2", "v1.2"),
            node_with_label("2", "", "v2", "v2"),
        ];
        let placement = layout(&mut nodes, 3);
        for node in &nodes {
            assert_eq!(node.half_w, NODE_HALF_H, "{}", node.version.id);
            assert_eq!(node.x % GRID, 0.0, "{} x={}", node.version.id, node.x);
            assert_eq!(node.y % GRID, 0.0, "{} y={}", node.version.id, node.y);
        }
        assert_eq!(nodes[1].x - nodes[0].x, COL_PITCH);
        // The gutter centre sits between the two columns' edges.
        assert_eq!(
            placement.gutters[0],
            (nodes[0].x + NODE_HALF_H + nodes[1].x - NODE_HALF_H) / 2.0
        );
    }

    #[test]
    fn long_label_widens_its_node_and_its_column_only() {
        let mut nodes = vec![
            node_with_label("2", "", "v2", "v2"),
            node_with_label("2.1", "2", ".1", "v2.1"),
            node_with_label("2.1.11", "2.1", ".11", "v2.1.11"),
            node_with_label("2.1.12", "2.1", ".12", "v2.1.12"),
        ];
        layout(&mut nodes, 4);
        assert!(nodes[2].half_w > NODE_HALF_H);
        assert_eq!(nodes[1].half_w, NODE_HALF_H);
        // Columns 0 and 1 keep the square pitch; the wide column is a whole
        // number of cells wide and the widened nodes stay centred on a dot.
        assert_eq!(nodes[1].x - nodes[0].x, COL_PITCH);
        assert_eq!(nodes[2].x % GRID, 0.0);
        assert_eq!(nodes[3].x, nodes[2].x);
    }

    #[test]
    fn fitting_drops_columns_when_a_wide_column_overflows() {
        // Three top-level versions with very long ids: they fit three
        // columns as squares but not once widened.
        let mut nodes = vec![
            node_with_label("1", "", "v1", "v1"),
            node_with_label("2", "", "v2", "v2"),
            node_with_label("3", "", "v33333333333", "v33333333333"),
        ];
        let width = 2.0 * 56.0 + 2.0 * COL_PITCH; // exactly three square columns
        assert_eq!(columns_for_width(width), 3);
        let fitted = layout_fitting(&mut nodes, width);
        assert!(fitted.columns < 3);
        assert!(extent(&nodes).0 <= width + 0.5);
    }

    #[test]
    fn extent_ends_half_a_cell_right_and_an_origin_margin_below() {
        assert_eq!(extent(&[]), (0.0, 0.0));
        let mut nodes = vec![node("1", ""), node("2", "")];
        layout(&mut nodes, 3);
        assert_eq!(
            extent(&nodes),
            (
                nodes[1].x + NODE_HALF_H + GRID / 2.0,
                nodes[0].y + NODE_HALF_H + (2.0 * GRID - NODE_HALF_H)
            )
        );
    }

    #[test]
    fn the_lattice_is_identical_at_every_zoom() {
        // The same history laid out for the same pane at three zoom levels:
        // the wrap width changes, but a node that stays in its column keeps
        // exactly the same coordinates, so nothing drifts against the dots.
        let pane = 610.0;
        let mut columns_seen = Vec::new();
        for zoom in [0.5_f32, 1.0, 2.0] {
            let mut nodes: Vec<GraphNode> = (1..=14).map(|n| node(&n.to_string(), "")).collect();
            let fitted = layout_fitting(&mut nodes, pane / zoom);
            columns_seen.push(fitted.columns);
            for node in &nodes {
                assert_eq!(node.x % GRID, 0.0, "{} x={}", node.version.id, node.x);
                assert_eq!(node.y % GRID, 0.0, "{} y={}", node.version.id, node.y);
                // Position follows only the cell, never the zoom.
                assert_eq!(node.x, 2.0 * GRID + node.col as f32 * COL_PITCH);
                assert_eq!(node.y, 2.0 * GRID + node.row as f32 * ROW_PITCH);
            }
        }
        // Zooming out fits more columns, zooming in fewer.
        assert!(columns_seen[0] > columns_seen[1] && columns_seen[1] > columns_seen[2]);
    }

    #[test]
    fn a_big_history_keeps_a_roughly_square_shape() {
        let mut nodes: Vec<GraphNode> = (1..=100).map(|n| node(&n.to_string(), "")).collect();
        let pane_w = 610.0;
        let fit_columns = layout_fitting(&mut nodes, pane_w).columns;

        let fitted = layout_for_pane(&mut nodes, pane_w);
        assert!(fitted.columns > fit_columns, "columns grew past the pane");
        // 100 versions: a 10 x 10 block, so neither scroll is pointless.
        assert_eq!(fitted.columns, 10);
        assert_eq!(rows_used(&nodes), 10);
        assert!(extent(&nodes).0 > pane_w, "wider than the pane");
    }

    #[test]
    fn a_history_that_fits_the_width_uses_it_all() {
        // 30 versions in a pane 7 columns wide: squarer would be 6, but the
        // pane's width wins, so the lattice spans it and scrolls vertically.
        let mut nodes: Vec<GraphNode> = (1..=30).map(|n| node(&n.to_string(), "")).collect();
        let pane_w = 610.0;
        let fitted = layout_for_pane(&mut nodes, pane_w);
        assert_eq!(fitted.columns, columns_for_width(pane_w));
        assert!(extent(&nodes).0 <= pane_w + 0.5, "no horizontal overflow");
        assert!(rows_used(&nodes) > 1, "rows to scroll through");
    }

    #[test]
    fn a_small_history_still_fits_the_pane_exactly() {
        let mut nodes: Vec<GraphNode> = (1..=8).map(|n| node(&n.to_string(), "")).collect();
        let pane_w = 610.0;
        let fitted = layout_for_pane(&mut nodes, pane_w);
        assert_eq!(fitted.columns, columns_for_width(pane_w));
        assert!(extent(&nodes).0 <= pane_w + 0.5, "no horizontal overflow");
    }

    #[test]
    fn columns_run_to_the_right_edge() {
        // Six square columns end at 56 + 5*84 + 21 = 497; with the half-cell
        // clearance they need 511px, and a seventh needs 595.
        assert_eq!(columns_for_width(510.0), 5);
        assert_eq!(columns_for_width(511.0), 6);
        assert_eq!(columns_for_width(594.0), 6);
        assert_eq!(columns_for_width(0.0), 1);
    }
}
