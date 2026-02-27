use bevy::prelude::*;

use super::screen_space::norm_to_viewport;
use super::screen_space::screen_edge_center;
use crate::fit::Edge;
use crate::support::ScreenSpaceBounds;

/// Font size used for all debug labels.
const LABEL_FONT_SIZE: f32 = 11.0;
/// Pixel offset used to keep labels off line endpoints and screen edges.
const LABEL_PIXEL_OFFSET: f32 = 8.0;

/// Component marking margin percentage labels.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct MarginLabel {
    pub edge: Edge,
}

/// Component marking the "screen space bounds" label.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct BoundsLabel;

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
fn apply_margin_label_anchor(node: &mut Node, edge: Edge, screen_pos: Vec2, viewport_size: Vec2) {
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
fn margin_label_node(edge: Edge, screen_pos: Vec2, viewport_size: Vec2) -> Node {
    let mut node = Node {
        position_type: PositionType::Absolute,
        ..default()
    };
    apply_margin_label_anchor(&mut node, edge, screen_pos, viewport_size);
    node
}

/// Updates an existing margin label or creates a new one.
pub fn update_or_create_margin_label(
    commands: &mut Commands,
    label_query: &mut Query<(Entity, &MarginLabel, &mut Text, &mut Node, &mut TextColor)>,
    edge: Edge,
    text: String,
    color: Color,
    screen_pos: Vec2,
    viewport_size: Vec2,
) {
    let mut found = false;
    for (_, label, mut label_text, mut node, mut text_color) in label_query {
        if label.edge == edge {
            **label_text = text.clone();
            text_color.0 = color;
            apply_margin_label_anchor(&mut node, edge, screen_pos, viewport_size);
            found = true;
            break;
        }
    }

    if !found {
        commands.spawn((
            Text::new(text),
            TextFont {
                font_size: LABEL_FONT_SIZE,
                ..default()
            },
            TextColor(color),
            margin_label_node(edge, screen_pos, viewport_size),
            MarginLabel { edge },
        ));
    }
}

#[allow(clippy::type_complexity)]
/// Updates an existing bounds label position or creates a new one.
pub fn update_or_create_bounds_label(
    commands: &mut Commands,
    bounds_query: &mut Query<(Entity, &mut Node), (With<BoundsLabel>, Without<MarginLabel>)>,
    screen_pos: Vec2,
) {
    if let Ok((_, mut node)) = bounds_query.single_mut() {
        node.left = Val::Px(screen_pos.x);
        node.top = Val::Px(screen_pos.y);
    } else {
        commands.spawn((
            Text::new("screen space bounds"),
            TextFont {
                font_size: LABEL_FONT_SIZE,
                ..default()
            },
            TextColor(Color::srgb(1.0, 1.0, 0.0)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(screen_pos.x),
                top: Val::Px(screen_pos.y),
                ..default()
            },
            BoundsLabel,
        ));
    }
}

/// Despawns all active margin labels.
pub fn cleanup_margin_labels(
    mut commands: Commands,
    label_query: Query<Entity, With<MarginLabel>>,
) {
    for entity in &label_query {
        commands.entity(entity).despawn();
    }
}
