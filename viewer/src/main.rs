mod light;
mod rotator;
mod viewer;

use light::Light;
use rotator::Rotator;
use viewer::{ImageSequence, Viewer};

const IMAGE_SEQUENCE_FOLDER_1: &str = "./sequences/lens";
const IMAGE_SEQUENCE_FOLDER_2: &str = "./sequences/remote";

const WINDOW_1_W: usize = 1920;
const WINDOW_1_H: usize = 1080;
const WINDOW_1_X: isize = 0;
const WINDOW_1_Y: isize = 0;

const WINDOW_2_W: usize = 1920;
const WINDOW_2_H: usize = 1080;
const WINDOW_2_X: isize = 1920;
const WINDOW_2_Y: isize = 0;

fn main() {
    let seq1 = ImageSequence::load(IMAGE_SEQUENCE_FOLDER_1, WINDOW_1_W, WINDOW_1_H);
    let seq2 = ImageSequence::load(IMAGE_SEQUENCE_FOLDER_2, WINDOW_2_W, WINDOW_2_H);
    println!("Loaded {} frames for sequence 1, {} frames for sequence 2", seq1.frame_count(), seq2.frame_count());

    let rotator = Rotator::start();
    let light = Light::new();

    let mut viewer = Viewer::new(
        seq1, (WINDOW_1_W, WINDOW_1_H), (WINDOW_1_X, WINDOW_1_Y),
        seq2, (WINDOW_2_W, WINDOW_2_H), (WINDOW_2_X, WINDOW_2_Y),
    );

    while viewer.is_open() {
        let angle = rotator.angle();
        println!("Angle: {:.2}", angle);
        viewer.render(angle);
        light.update(angle);
    }
}
