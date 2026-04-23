pub const TARGET_FRAME: usize = 30;

// Scale/brightness: starts ~150°, fully active at ~20°
const SCALE_INNER: f64 = 0.06;
const SCALE_OUTER: f64 = 0.42;

// Diamond reflections: much narrower, starts ~35°, fully active at ~7°
const DIAMOND_INNER: f64 = 0.02;
const DIAMOND_OUTER: f64 = 0.10;

// Returns 1.0 within inner zone, 0.0 beyond outer zone, linear between.
fn eased_proximity(frame_index: usize, frame_count: usize, inner_fraction: f64, outer_fraction: f64) -> f64 {
    if frame_count == 0 { return 0.0; }
    let total = frame_count as isize;
    let fwd = ((frame_index as isize - TARGET_FRAME as isize).rem_euclid(total)) as f64;
    let bwd = frame_count as f64 - fwd;
    let dist = fwd.min(bwd);

    let inner = inner_fraction * frame_count as f64;
    let outer = outer_fraction * frame_count as f64;

    if dist <= inner { return 1.0; }
    if dist >= outer { return 0.0; }
    1.0 - (dist - inner) / (outer - inner)
}

pub fn eased_proximity_scale(frame_index: usize, frame_count: usize) -> f64 {
    eased_proximity(frame_index, frame_count, SCALE_INNER, SCALE_OUTER)
}

pub fn eased_proximity_diamond(frame_index: usize, frame_count: usize) -> f64 {
    eased_proximity(frame_index, frame_count, DIAMOND_INNER, DIAMOND_OUTER)
}
