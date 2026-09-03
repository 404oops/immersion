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
//! Geometry is in 28px grid cells at 100% zoom: every node centre lands on
//! a grid dot, a node spans 3 x 1.5 cells, columns are 5 cells apart and
//! rows 3, and links run through the two-cell gutter between columns.

use std::collections::HashMap;

use musit_core::backend::{VersionEntry, VersionGraphNode};

/// Dot-grid pitch at 100% zoom.
pub const GRID: f32 = 28.0;
pub const NODE_HALF_W: f32 = 1.5 * GRID;
pub const NODE_HALF_H: f32 = 0.75 * GRID;
pub const COL_PITCH: f32 = 5.0 * GRID;
pub const ROW_PITCH: f32 = 3.0 * GRID;
/// Centre of the first column and row: half a cell of clearance beside the
/// first node and room above it for the CURRENT badge.
const ORIGIN: f32 = 2.0 * GRID;

/// A version placed on the graph pane.
#[derive(Clone, Debug)]
pub struct GraphNode {
    pub version: VersionEntry,
    /// Parent version id; empty for a top-level version.
    pub parent_id: String,
    /// Centre, in content pixels at 100% zoom.
    pub x: f32,
    pub y: f32,
}

impl From<VersionGraphNode> for GraphNode {
    fn from(node: VersionGraphNode) -> Self {
        Self {
            version: node.version,
            parent_id: node.parent_id,
            x: 0.0,
            y: 0.0,
        }
    }
}

/// Columns that fit a pane `viewport_width` px wide at 100% zoom while the
/// last column keeps the same clearance from the pane edge as the first.
pub fn columns_for_width(viewport_width: f32) -> usize {
    let usable = viewport_width - 2.0 * ORIGIN;
    if usable < 0.0 {
        1
    } else {
        (usable / COL_PITCH) as usize + 1
    }
}

/// Assigns every node's centre. `nodes` must list parents before their
/// children with siblings in display order (the order the backend emits);
/// that order is preserved. A node whose parent is missing, or listed after
/// it, is laid out as a root.
pub fn layout(nodes: &mut [GraphNode], columns: usize) {
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
}

fn place(
    nodes: &mut [GraphNode],
    children: &[Vec<usize>],
    height: &[usize],
    index: usize,
    column: usize,
    row: usize,
) {
    nodes[index].x = ORIGIN + column as f32 * COL_PITCH;
    nodes[index].y = ORIGIN + row as f32 * ROW_PITCH;
    let mut child_row = row;
    for &child in &children[index] {
        place(nodes, children, height, child, column + 1, child_row);
        child_row += height[child];
    }
}

/// Scrollable content size: the far node edges plus the same clearance the
/// layout leaves before the first node.
pub fn extent(nodes: &[GraphNode]) -> (f32, f32) {
    nodes
        .iter()
        .fold((0.0_f32, 0.0_f32), |(width, height), node| {
            (width.max(node.x + ORIGIN), height.max(node.y + ORIGIN))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, parent: &str) -> GraphNode {
        GraphNode {
            version: VersionEntry {
                id: id.to_string(),
                ..VersionEntry::default()
            },
            parent_id: parent.to_string(),
            x: 0.0,
            y: 0.0,
        }
    }

    /// (column, row) a laid-out node landed on.
    fn cell(node: &GraphNode) -> (usize, usize) {
        (
            ((node.x - ORIGIN) / COL_PITCH).round() as usize,
            ((node.y - ORIGIN) / ROW_PITCH).round() as usize,
        )
    }

    fn cells(nodes: &[GraphNode]) -> Vec<(&str, (usize, usize))> {
        nodes
            .iter()
            .map(|node| (node.version.id.as_str(), cell(node)))
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
        // Depth-first order, as the backend emits it.
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
                // v1's block is two columns wide, so v2 sits in the third...
                ("2", (2, 0)),
                // ...and the next line starts below the two-row block.
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
                // Three columns wide: does not fit after v1, so it wraps.
                ("2", (0, 1)),
                ("2.1", (1, 1)),
                ("2.1.2", (2, 1)),
                ("2.1.3", (2, 2)),
                // 2.1's fan is two rows tall, so 2.2 sits below it.
                ("2.2", (1, 3)),
                // The block is three rows tall (rows 1-3).
                ("3", (0, 4)),
                ("4", (1, 4)),
            ]
        );
    }

    #[test]
    fn block_wider_than_the_pane_overflows_from_the_line_start() {
        let mut nodes = vec![node("1", ""), node("1.1", "1"), node("2", "")];
        layout(&mut nodes, 1);
        assert_eq!(
            cells(&nodes),
            vec![("1", (0, 0)), ("1.1", (1, 0)), ("2", (0, 1))]
        );
    }

    #[test]
    fn missing_parent_makes_a_root() {
        let mut nodes = vec![node("1", ""), node("5.1", "5")];
        layout(&mut nodes, 4);
        assert_eq!(cells(&nodes), vec![("1", (0, 0)), ("5.1", (1, 0))]);
    }

    #[test]
    fn columns_match_pane_width() {
        assert_eq!(columns_for_width(0.0), 1);
        assert_eq!(columns_for_width(300.0), 2);
        assert_eq!(columns_for_width(440.0), 3);
        assert_eq!(columns_for_width(580.0), 4);
    }

    #[test]
    fn nodes_sit_on_grid_dots() {
        let mut nodes = vec![node("1", ""), node("1.1", "1"), node("2", "")];
        layout(&mut nodes, 3);
        for node in &nodes {
            assert_eq!(node.x % GRID, 0.0, "{} x={}", node.version.id, node.x);
            assert_eq!(node.y % GRID, 0.0, "{} y={}", node.version.id, node.y);
        }
    }

    #[test]
    fn extent_pads_the_far_edges_like_the_origin() {
        assert_eq!(extent(&[]), (0.0, 0.0));
        let mut nodes = vec![node("1", ""), node("2", "")];
        layout(&mut nodes, 3);
        assert_eq!(
            extent(&nodes),
            (ORIGIN + COL_PITCH + ORIGIN, ORIGIN + ORIGIN)
        );
    }
}
