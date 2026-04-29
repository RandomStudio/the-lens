mod config;
mod debug_display;
mod display;
mod easing;
mod image_sequence;
mod light;
mod mqtt_receiver;
mod mqtt_sender;
mod receiver;
mod rotator;
mod surface;

use config::{Config, resolve_index_transform};
use debug_display::DebugDisplay;
use display::Display;
use mqtt_receiver::MqttReceiver;
use mqtt_sender::MqttSender;
use receiver::AngleReceiver;
use rotator::Rotator;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::WindowId;

enum Mode {
    Normal {
        display: Display,
        sender: MqttSender,
        receiver: Box<dyn AngleReceiver>,
    },
    Debug {
        display: DebugDisplay,
        receiver: Box<dyn AngleReceiver>,
    },
}

struct App {
    cfg: Config,
    transform: fn(isize, isize) -> isize,
    mode: Option<Mode>,
    last_angle: f64,
}

impl App {
    fn new(cfg: Config) -> Self {
        let transform = resolve_index_transform(&cfg.index_transform);
        Self { cfg, transform, mode: None, last_angle: 0.0 }
    }

    fn handles_window(&self, id: WindowId) -> bool {
        match &self.mode {
            Some(Mode::Normal { display, .. }) => display.handles_window(id),
            Some(Mode::Debug { display, .. }) => display.handles_window(id),
            None => false,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.mode.is_some() { return; }

        if self.cfg.is_debug_screen {
            let receiver: Box<dyn AngleReceiver> = Box::new(MqttReceiver::new(self.cfg.mqtt.clone()));
            let display = DebugDisplay::new(
                event_loop,
                &self.cfg.sequence_path,
                self.transform,
                self.cfg.min_scale,
                self.cfg.max_scale,
                self.cfg.brightest_brightness,
                self.cfg.easing_multiplier,
            );
            self.mode = Some(Mode::Debug { display, receiver });
        } else {
            let receiver: Box<dyn AngleReceiver> = Box::new(Rotator::new());
            let sender = MqttSender::new(&self.cfg.mqtt);
            let display = Display::new(
                event_loop,
                &self.cfg.sequence_path,
                &self.cfg.diamond_path,
                self.transform,
                self.cfg.min_scale,
                self.cfg.max_scale,
                self.cfg.brightest_brightness,
                self.cfg.easing_multiplier,
            );
            self.mode = Some(Mode::Normal { display, sender, receiver });
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if !self.handles_window(window_id) { return; }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key, state: ElementState::Pressed, .. },
                ..
            } => {
                let exit = matches!(logical_key, Key::Named(NamedKey::Escape))
                    || matches!(logical_key.as_ref(), Key::Character("q") | Key::Character("Q"));
                if exit {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                match &mut self.mode {
                    Some(Mode::Normal { display, .. }) => {
                        display.resize_window(window_id, size.width, size.height);
                    }
                    Some(Mode::Debug { display, .. }) => {
                        display.resize_window(window_id, size.width, size.height);
                    }
                    None => {}
                }
            }
            WindowEvent::RedrawRequested => {
                let angle = self.last_angle;
                match &mut self.mode {
                    Some(Mode::Normal { display, .. }) => display.render_window(window_id, angle),
                    Some(Mode::Debug { display, .. }) => display.render_window(window_id, angle),
                    None => {}
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        match &mut self.mode {
            Some(Mode::Normal { display, sender, receiver }) => {
                let angle = receiver.angle();
                sender.publish_angle(angle);
                self.last_angle = angle;
                display.request_redraws();
            }
            Some(Mode::Debug { display, receiver }) => {
                self.last_angle = receiver.angle();
                display.request_redraws();
            }
            None => {}
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(Mode::Normal { display, .. }) = &self.mode {
            display.turn_off_light();
        }
    }
}

fn main() {
    let cfg = Config::load("./config.json");
    let event_loop = EventLoop::new().expect("failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(cfg);
    event_loop.run_app(&mut app).expect("event loop error");
}
