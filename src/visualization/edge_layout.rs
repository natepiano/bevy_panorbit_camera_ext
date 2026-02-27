use bevy::prelude::*;

use super::geometry::norm_to_viewport;
use super::geometry::screen_edge_center;
use crate::fit::Edge;
use crate::support::ScreenSpaceBounds;

/// Font size used for all debug labels.
pub const LABEL_FONT_SIZE: f32 = 11.0;
/// Pixel offset used to keep labels off line endpoints and screen edges.
pub const LABEL_PIXEL_OFFSET: f32 = 8.0;

/// Calculates the viewport pixel position for a margin label, offset by a fixed
/// number of pixels from the screen-edge endpoint of the margin line.
pub fn calculate_label_pixel_position(
    edge: Edge,
    bounds: &ScreenSpaceBounds,
    viewport_size: Vec2,
) -> Vec2 {
    let (screen_x, screen_y) = screen_edge_center(bounds, edge);
    let px = norm_to_viewport(
        screen_x,
        screen_y,
        bounds.half_extent_x,
        bounds.half_extent_y,
        viewport_size,
    );

    // Left/Right labels sit above the horizontal line;
    // Top/Bottom labels sit beside the vertical line with pixel offsets.
    let above_line = px.y - LABEL_FONT_SIZE - LABEL_PIXEL_OFFSET;

    match edge {
        Edge::Left => Vec2::new(LABEL_PIXEL_OFFSET, above_line),
        Edge::Right => Vec2::new(viewport_size.x - LABEL_PIXEL_OFFSET, above_line),
        Edge::Top => Vec2::new(px.x + LABEL_PIXEL_OFFSET, LABEL_PIXEL_OFFSET),
        Edge::Bottom => Vec2::new(
            px.x + LABEL_PIXEL_OFFSET,
            viewport_size.y - LABEL_PIXEL_OFFSET,
        ),
    }
}

/// Returns the final viewport position for the "screen space bounds" label.
pub fn bounds_label_position(upper_left: Vec2) -> Vec2 {
    Vec2::new(
        upper_left.x + LABEL_PIXEL_OFFSET,
        upper_left.y - LABEL_FONT_SIZE - LABEL_PIXEL_OFFSET,
    )
}

/// Applies anchored placement for a margin label node based on edge semantics.
pub fn apply_margin_label_anchor(
    node: &mut Node,
    edge: Edge,
    screen_pos: Vec2,
    viewport_size: Vec2,
) {
    match edge {
        Edge::Left | Edge::Top => {
            node.left = Val::Px(screen_pos.x);
            node.top = Val::Px(screen_pos.y);
            node.right = Val::Auto;
            node.bottom = Val::Auto;
        },
        Edge::Right => {
            node.right = Val::Px(viewport_size.x - screen_pos.x);
            node.top = Val::Px(screen_pos.y);
            node.left = Val::Auto;
            node.bottom = Val::Auto;
        },
        Edge::Bottom => {
            node.left = Val::Px(screen_pos.x);
            node.bottom = Val::Px(viewport_size.y - screen_pos.y);
            node.right = Val::Auto;
            node.top = Val::Auto;
        },
    }
}

/// Builds an anchored node for a new margin label.
pub fn margin_label_node(edge: Edge, screen_pos: Vec2, viewport_size: Vec2) -> Node {
    let mut node = Node {
        position_type: PositionType::Absolute,
        ..default()
    };
    apply_margin_label_anchor(&mut node, edge, screen_pos, viewport_size);
    node
}
