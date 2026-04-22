// Frame index around which all easing effects peak
pub const TARGET_FRAME: usize = 30;

// t^5 curve: stays near zero for most of the range, rises sharply near target
pub fn eased_proximity(frame_index: usize, frame_count: usize) -> f64 {
    if frame_count == 0 { return 0.0; }
    let total = frame_count as isize;
    let fwd = ((frame_index as isize - TARGET_FRAME as isize).rem_euclid(total)) as f64;
    let bwd = frame_count as f64 - fwd;
    let dist = fwd.min(bwd);
    let proximity = 1.0 - (dist / (frame_count as f64 / 2.0)).clamp(0.0, 1.0);
    proximity.powf(5.0)
}
