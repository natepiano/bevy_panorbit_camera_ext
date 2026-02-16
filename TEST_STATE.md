# Zoom Test State - Saved for Reproducible Testing

## Donut Entity: 4294966173
- Position: [0, 0, 0]
- Rotation: [0, 0, 0, 1]
- Scale: [1, 1, 1]

## Camera Entity: 4294967182

### Transform
- Position: [-984.8433837890624, 293.965087890625, -1260.0111083984375]
- Rotation: [-0.06510020792484283, -0.8097250461578369, -0.09152229875326157, 0.5759609341621399]

### PanOrbitCamera
- Focus: [294.9356384277344, -16.251224517822266, -815.5670776367188]
- Radius: 1389.8197021484375
- Yaw: -1.9050476551055908
- Pitch: 0.22510236501693728

## Reset Commands

```rust
// Reset donut position
world_insert_components(4294966173, Transform {
    translation: [0, 0, 0],
    rotation: [0, 0, 0, 1],
    scale: [1, 1, 1]
})

// Reset camera to test state
StartAnimation {
    entity: 4294967182,
    moves: [{
        target_translation: [-984.8433837890624, 293.965087890625, -1260.0111083984375],
        target_focus: [294.9356384277344, -16.251224517822266, -815.5670776367188],
        target_radius: 1389.8197021484375,
        target_yaw: -1.9050476551055908,
        target_pitch: 0.22510236501693728,
        duration_ms: 1,
        easing: "Linear"
    }]
}

// Then trigger zoom
ZoomToFitMesh { entity: 4294967182, target_entity: 4294966173 }
```

## Notes
- Camera is looking at [294.93, -16.25, -815.56], which is ~871 units away from donut at [0, 0, 0]
- This represents a challenging test case where focus is far from target
- Good for testing smooth coordinated convergence of focus + radius + orientation
