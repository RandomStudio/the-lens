mod light;
mod mqtt_rotator;
mod rotator;
mod viewer;

use light::Light;
use mqtt_rotator::MqttRotator;
use rotator::Rotator;
use viewer::{ImageSequence, Viewer};

const IMAGE_SEQUENCE_FOLDER_1: &str = "./sequences/lens";
const IMAGE_SEQUENCE_FOLDER_2: &str = "./sequences/remote";

const LENS_DISPLAY: usize = 1;
const REMOTE_DISPLAY: usize = 0;

fn main() {
    let seq1 = ImageSequence::load(IMAGE_SEQUENCE_FOLDER_1);
    let seq2 = ImageSequence::empty(); //ImageSequence::load(IMAGE_SEQUENCE_FOLDER_2);
    println!(
        "[INFO] Sequence 1: {} frames, Sequence 2: {} frames",
        seq1.frame_count(),
        seq2.frame_count()
    );

    let rotator = Rotator::new();
    let light = Light::new();
    let mqtt = MqttRotator::new(&rotator).start();

    let mut viewer = Viewer::new(seq1, LENS_DISPLAY, seq2, REMOTE_DISPLAY);

    while viewer.is_open() {
        let angle = rotator.angle();
        viewer.render(angle);
        light.update(angle);
        mqtt.update(angle);
    }
}
