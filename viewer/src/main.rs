mod light;
mod rotator;
mod viewer;

use light::Light;
use rotator::Rotator;
use viewer::{ImageSequence, Viewer};

const IMAGE_SEQUENCE_FOLDER_1: &str = "./sequences/lens";
const IMAGE_SEQUENCE_FOLDER_2: &str = "./sequences/remote";

const DISPLAY_1: usize = 0;
const DISPLAY_2: usize = 1;

fn main() {
    let seq1 = ImageSequence::load(IMAGE_SEQUENCE_FOLDER_1);
    let seq2 = ImageSequence::load(IMAGE_SEQUENCE_FOLDER_2);
    println!(
        "[INFO] Sequence 1: {} frames, Sequence 2: {} frames",
        seq1.frame_count(),
        seq2.frame_count()
    );

    let rotator = Rotator::start();
    let light = Light::new();

    let mut viewer = Viewer::new(seq1, DISPLAY_1, seq2, DISPLAY_2);

    while viewer.is_open() {
        let angle = rotator.angle();
        viewer.render(angle);
        light.update(angle);
    }
}
