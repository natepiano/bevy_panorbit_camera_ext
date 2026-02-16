# Zoom-to-Fit Focus Oscillation Fix Procedure

## Problem
ZoomToFitMesh on spaceship causes focus to oscillate between 4 values while regular zoom (Z key) keeps focus stable.

## Baseline Values

### Camera Reset Position
- Entity: (find dynamically)
- Focus: [0, 0, 0]
- Yaw: 0
- Pitch: 0
- Radius: 3211.302490234375

### Canonical Zoom Baseline (GOOD)
- Location: `/var/folders/rf/twhh0jfd243fpltn5k0w1t980000gn/T/canonical_zoom_baseline.log`
- Focus: [0, 0, 0] throughout (no oscillation)
- Radius: 3211.302490234375 → 350.07061767578125 (smooth convergence)

---

## Test Procedure (Each Iteration)

### Setup
1. Kill app: `mcp__brp__brp_shutdown` (nateroids, port 20000)
2. Make fix in `/Users/natemccoy/rust/bevy_panorbit_camera_ext/src/zoom.rs`
3. Build: `cd /Users/natemccoy/rust/bevy_panorbit_camera_ext && cargo build`
4. Launch: `mcp__brp__brp_launch_bevy_app` (nateroids, port 20000)
5. Sleep 10 seconds
6. Set window title: `mcp__brp__brp_extras_set_window_title` ("nateroids - 20000")
7. Send Esc to pause: `mcp__brp__brp_extras_send_keys` (["Escape"])

### Test 1 - Regular Zoom (Baseline Validation)

**Purpose**: Ensure fix doesn't break existing zoom functionality

1. Reset camera to baseline using StartAnimation event:
   - Find camera entity
   - Create CameraMove: target_translation=[0, 0, 3211.302490234375], target_focus=[0, 0, 0], duration_ms=1, easing=Linear
   - Trigger StartAnimation event with this move
   - Sleep 1 second

2. Find camera entity (may have changed after reset)

3. Set watch on camera:
   - `mcp__brp__world_get_components_watch` on camera entity
   - Track: ["bevy_panorbit_camera::PanOrbitCamera"]

4. Send Z key: `mcp__brp__brp_extras_send_keys` (["KeyZ"])

5. Sleep 2 seconds

6. Analyze watch:
   ```bash
   python3 /Users/natemccoy/rust/bevy_panorbit_camera_ext/detect_oscillation.py <logfile>
   ```
   - Exit code 0 = PASS (converging)
   - Exit code 1 = FAIL (oscillating)

7. Stop watch: `mcp__brp__brp_stop_watch`

**PASS Criteria**:
- Oscillation detector reports "CONVERGING" or "CONVERGED"
- Focus: [0, 0, 0] stable throughout
- Radius: Smoothly converges 3211.302490234375 → 350.07061767578125

**FAIL Criteria**:
- Oscillation detector reports "OSCILLATING" with cycle pattern
- Focus cycling between multiple values
- → Fix broke regular zoom, revert and try different approach

---

### Test 2 - Spaceship Zoom (Fix Validation)

**Purpose**: Verify fix eliminates spaceship focus oscillation

1. Reset camera to baseline (same as Test 1 step 1)

2. Find camera entity

3. Find spaceship entity:
   ```bash
   curl -s -X POST http://127.0.0.1:20000/jsonrpc -H "Content-Type: application/json" \
     -d '{"jsonrpc": "2.0", "id": 1, "method": "world.query", "params": {"filter": {}, "data": {"components": ["bevy_ecs::name::Name"]}}}' \
     | jq -r '.result[] | select(.components."bevy_ecs::name::Name" == "Spaceship") | .entity'
   ```

4. Set watch on camera (new watch):
   - `mcp__brp__world_get_components_watch` on camera entity

5. Trigger ZoomToFitMesh:
   - Event: `bevy_panorbit_camera_ext::extension::ZoomToFitMesh`
   - Value: {"entity": <camera_entity>, "target_entity": <spaceship_entity>}

6. Sleep 2 seconds

7. Analyze watch:
   ```bash
   python3 /Users/natemccoy/rust/bevy_panorbit_camera_ext/detect_oscillation.py <logfile>
   ```
   - Exit code 0 = PASS (converging)
   - Exit code 1 = FAIL (oscillating)

8. Stop watch: `mcp__brp__brp_stop_watch`

**PASS Criteria**:
- Oscillation detector reports "CONVERGING" or "CONVERGED"
- Focus: Smooth convergence to stable value
- NO cycling between values (e.g., 8-value cycle: -12.32, -13.02, -13.65, -14.23, -14.74, -15.99, -16.91, -17.59)
- Radius: Smooth convergence (secondary check)

**FAIL Criteria**:
- Oscillation detector reports "OSCILLATING" with cycle pattern
- Focus cycling between multiple values repeatedly
- → Try different fix and repeat

---

## Success Criteria
- Test 1 PASS (preserve baseline zoom)
- Test 2 PASS (fix spaceship oscillation)

---

## Fix Attempts Log

### Attempt 1: Adaptive Convergence Rate for Radius
**Date**: 2026-02-14
**File**: `/Users/natemccoy/rust/bevy_panorbit_camera_ext/src/zoom.rs`
**Change**: Lines 278-295
- Added adaptive convergence rate based on radius_change_ratio and focus_change_ratio
- Speed up to 0.8 when changes < 5% of current radius
- Keep base rate (0.30) for large changes

**Result**:
- Test 1: NOT TESTED YET
- Test 2: FAILED - Focus still oscillates between 4 values
- **Issue**: Adaptive rate applied to overall convergence, but focus calculation itself creates oscillation
- **Root Cause**: `calculate_target_focus` recalculates different target each frame (feedback loop)

---

### Attempt 2: Dampen Focus Correction in Phase 2
**Date**: 2026-02-14
**File**: `/Users/natemccoy/rust/bevy_panorbit_camera_ext/src/zoom.rs`
**Change**: Line 342
- Changed `current_focus + focus_correction` to `current_focus + focus_correction * 0.3`
- Apply only 30% of focus correction per frame to prevent overshooting

**Result**:
- Test 1: PASS - Regular zoom still converges correctly (focus stable at [0, 0, 0])
- Test 2: FAIL - Spaceship zoom still oscillates
  - Oscillation pattern: 8-value cycle (-12.32, -13.02, -13.65, -14.23, -14.74, -15.99, -16.91, -17.59)
  - Detected by oscillation detector script
- **Issue**: Dampening reduced but didn't eliminate oscillation
- **Root Cause**: Still investigating - dampening factor of 0.3 insufficient

---

### Attempt 3: Use Actual Corners Center (FIX SUCCESSFUL)
**Date**: 2026-02-14
**File**: `/Users/natemccoy/rust/bevy_panorbit_camera_ext/src/zoom.rs`
**Changes**:
- Line 272-278: Pass `&zoom_state.target_corners` to `calculate_target_focus`
- Line 322-330: Add `corners` parameter and calculate actual center: `corners.iter().sum::<Vec3>() / 8.0`
- **Root Cause**: Function was hardcoded to center on `Vec3::ZERO`, but spaceship is at different position
- **Fix**: Calculate actual center of bounding box corners for proper focus targeting

**Result**:
- Test 1: ✅ PASS - CONVERGED at 0.0 (regular zoom unaffected)
- Test 2: ✅ PASS - CONVERGED at -19.55 (oscillation eliminated!)
- **Status**: PROBLEM SOLVED

Both tests pass oscillation detector. Spaceship zoom now smoothly converges without cycling.
