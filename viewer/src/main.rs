mod light;
mod mqtt_rotator;
mod rotator;
mod viewer;

use light::Light;
use mqtt_rotator::MqttRotator;
use rotator::Rotator;
use viewer::{ImageSequence, Viewer};

const LENS_DISPLAY: usize = 1;
const REMOTE_DISPLAY: usize = 0;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let get_arg = |flag: &str| -> Option<String> {
        args.windows(2)
            .find(|w| w[0] == flag)
            .map(|w| w[1].clone())
    };
    let username = get_arg("--username");
    let password = get_arg("--password");

    let seq1 = ImageSequence::load("./sequences/lens", |index, total| total - index - (total / 4))
        .hue_shift(0);
    let seq2 = ImageSequence::empty(); //ImageSequence::load(IMAGE_SEQUENCE_FOLDER_2);
    println!(
        "[INFO] Sequence 1: {} frames, Sequence 2: {} frames",
        seq1.frame_count(),
        seq2.frame_count()
    );

    let rotator = Rotator::new();
    let light = Light::new();
    let mqtt = MqttRotator::new(username, password);

    let mut viewer = Viewer::new(seq1, LENS_DISPLAY, seq2, REMOTE_DISPLAY);

    while viewer.is_open() {
        let angle = rotator.angle();
        viewer.render(angle);
        light.update(angle);
        mqtt.update(angle);
    }
}
