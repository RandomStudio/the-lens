// Frame index around which all easing effects peak
pub const TARGET_FRAME: usize = 30;

// Frames either side of target that are fully at peak (dead zone)
const DEAD_ZONE_FRAMES: f64 = 10.0;

// Within the dead zone: eased = 1.0 (lights fully off, diamond fully visible)
// Outside: fades with a t^3 curve over the remaining range — wider/softer than the old t^5
pub fn eased_proximity(frame_index: usize, frame_count: usize) -> f64 {
    if frame_count == 0 { return 0.0; }
    let total = frame_count as isize;
    let fwd = ((frame_index as isize - TARGET_FRAME as isize).rem_euclid(total)) as f64;
    let bwd = frame_count as f64 - fwd;
    let dist = fwd.min(bwd);

    if dist <= DEAD_ZONE_FRAMES {
        return 1.0;
    }

    let fade_range = (frame_count as f64 / 2.0) - DEAD_ZONE_FRAMES;
    let fade_dist = dist - DEAD_ZONE_FRAMES;
    let proximity = 1.0 - (fade_dist / fade_range).clamp(0.0, 1.0);
    proximity.powf(3.0)
}
