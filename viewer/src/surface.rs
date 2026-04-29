use std::sync::Arc;
use pixels::wgpu;
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Fullscreen, Window};

pub struct WindowedSurface {
    pub window: Arc<Window>,
    pub pixels: Pixels<'static>,
    pub width: u32,
    pub height: u32,
}

pub fn sorted_monitors(event_loop: &ActiveEventLoop) -> Vec<MonitorHandle> {
    let mut monitors: Vec<_> = event_loop.available_monitors().collect();
    monitors.sort_by_key(|m| {
        let p = m.position();
        (p.x, p.y)
    });
    monitors
}

pub fn create_fullscreen_surface(
    event_loop: &ActiveEventLoop,
    title: &str,
    monitor: MonitorHandle,
) -> WindowedSurface {
    let size = monitor.size();
    let attrs = Window::default_attributes()
        .with_title(title)
        .with_decorations(false)
        .with_inner_size(PhysicalSize::new(size.width, size.height))
        .with_fullscreen(Some(Fullscreen::Borderless(Some(monitor))));
    build_surface(event_loop, attrs, size.width, size.height)
}

pub fn create_windowed_surface(
    event_loop: &ActiveEventLoop,
    title: &str,
    width: u32,
    height: u32,
) -> WindowedSurface {
    let attrs = Window::default_attributes()
        .with_title(title)
        .with_inner_size(PhysicalSize::new(width, height));
    build_surface(event_loop, attrs, width, height)
}

fn build_surface(
    event_loop: &ActiveEventLoop,
    attrs: winit::window::WindowAttributes,
    width: u32,
    height: u32,
) -> WindowedSurface {
    let window = Arc::new(event_loop.create_window(attrs).expect("failed to create window"));
    let actual = window.inner_size();
    let (w, h) = (actual.width.max(1), actual.height.max(1));
    let surface_texture = SurfaceTexture::new(w, h, Arc::clone(&window));
    let pixels = PixelsBuilder::new(w, h, surface_texture)
        .texture_format(wgpu::TextureFormat::Rgba8Unorm)
        .request_adapter_options(wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .build()
        .expect("failed to create Pixels");
    let _ = (width, height);
    WindowedSurface { window, pixels, width: w, height: h }
}

impl WindowedSurface {
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        let _ = self.pixels.resize_surface(width, height);
        let _ = self.pixels.resize_buffer(width, height);
        self.width = width;
        self.height = height;
    }

    pub fn write_rgb(&mut self, src: &[u32]) {
        let frame = self.pixels.frame_mut();
        let n = (self.width as usize * self.height as usize).min(src.len());
        for i in 0..n {
            let p = src[i];
            let off = i * 4;
            frame[off]     = ((p >> 16) & 0xff) as u8;
            frame[off + 1] = ((p >> 8) & 0xff) as u8;
            frame[off + 2] = (p & 0xff) as u8;
            frame[off + 3] = 0xff;
        }
    }

    pub fn present(&self) {
        if let Err(e) = self.pixels.render() {
            eprintln!("[Surface] render failed: {}", e);
        }
    }
}
