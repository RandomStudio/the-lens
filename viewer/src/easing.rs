pub const TARGET_FRAME: usize = 30;

const INNER_FRACTION: f64 = 0.05;
const OUTER_FRACTION: f64 = 0.30;

// Returns 1.0 within inner zone, 0.0 beyond outer zone, linear between.
pub fn eased_proximity(frame_index: usize, frame_count: usize) -> f64 {
    if frame_count == 0 { return 0.0; }
    let total = frame_count as isize;
    let fwd = ((frame_index as isize - TARGET_FRAME as isize).rem_euclid(total)) as f64;
    let bwd = frame_count as f64 - fwd;
    let dist = fwd.min(bwd);

    let inner = INNER_FRACTION * frame_count as f64;
    let outer = OUTER_FRACTION * frame_count as f64;

    if dist <= inner { return 1.0; }
    if dist >= outer { return 0.0; }
    1.0 - (dist - inner) / (outer - inner)
}
