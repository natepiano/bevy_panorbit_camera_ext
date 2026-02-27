use bevy::prelude::*;

use super::edge_layout::apply_margin_label_anchor;
use super::edge_layout::margin_label_node;
use super::edge_layout::LABEL_FONT_SIZE;
use crate::fit::Edge;

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
