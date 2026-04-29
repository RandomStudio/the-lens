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
    /// Pixel buffer dimensions — what `frame_mut()` exposes and what render code writes.
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
    buffer: Option<(u32, u32)>,
) -> WindowedSurface {
    let size = monitor.size();
    let attrs = Window::default_attributes()
        .with_title(title)
        .with_decorations(false)
        .with_inner_size(PhysicalSize::new(size.width, size.height))
        .with_fullscreen(Some(Fullscreen::Borderless(Some(monitor))));
    build_surface(event_loop, attrs, buffer)
}

pub fn create_windowed_surface(
    event_loop: &ActiveEventLoop,
    title: &str,
    width: u32,
    height: u32,
    buffer: Option<(u32, u32)>,
) -> WindowedSurface {
    let attrs = Window::default_attributes()
        .with_title(title)
        .with_inner_size(PhysicalSize::new(width, height));
    build_surface(event_loop, attrs, buffer)
}

fn build_surface(
    event_loop: &ActiveEventLoop,
    attrs: winit::window::WindowAttributes,
    buffer: Option<(u32, u32)>,
) -> WindowedSurface {
    let window = Arc::new(event_loop.create_window(attrs).expect("failed to create window"));
    let actual = window.inner_size();
    let (sw, sh) = (actual.width.max(1), actual.height.max(1));
    let (bw, bh) = buffer
        .map(|(w, h)| (w.max(1), h.max(1)))
        .unwrap_or((sw, sh));
    let surface_texture = SurfaceTexture::new(sw, sh, Arc::clone(&window));
    let pixels = PixelsBuilder::new(bw, bh, surface_texture)
        .texture_format(wgpu::TextureFormat::Rgba8Unorm)
        .request_adapter_options(wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .build()
        .expect("failed to create Pixels");
    let info = pixels.adapter().get_info();
    println!("[Surface] Adapter: {} ({:?}, backend: {:?}) — surface {}x{}, buffer {}x{}",
             info.name, info.device_type, info.backend, sw, sh, bw, bh);
    WindowedSurface { window, pixels, width: bw, height: bh }
}

impl WindowedSurface {
    /// Resize the on-screen surface only — the pixel buffer dimensions stay fixed
    /// (the GPU handles upscale/downscale to whatever surface size we present at).
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        let _ = self.pixels.resize_surface(width, height);
    }

    pub fn write_rgb(&mut self, src: &[u32]) {
        let frame = self.pixels.frame_mut();
        for (dst, &p) in frame.chunks_exact_mut(4).zip(src.iter()) {
            dst[0] = (p >> 16) as u8;
            dst[1] = (p >> 8) as u8;
            dst[2] = p as u8;
            dst[3] = 0xff;
        }
    }

    /// Composite `fg` (RGBA in 0xAARRGGBB) over `bg` (RGB) in a single pass,
    /// writing the result directly into the GPU frame buffer as RGBA bytes.
    pub fn write_composite_rgb(&mut self, bg: &[u32], fg: &[u32]) {
        let frame = self.pixels.frame_mut();
        let chunks = frame.chunks_exact_mut(4);
        let pairs = bg.iter().zip(fg.iter());
        for (dst, (&b, &f)) in chunks.zip(pairs) {
            let a = (f >> 24) & 0xff;
            let (r, g, bl) = if a == 0xff {
                ((f >> 16) as u8, (f >> 8) as u8, f as u8)
            } else if a == 0 {
                ((b >> 16) as u8, (b >> 8) as u8, b as u8)
            } else {
                let af = a;
                let inv = 255 - af;
                let fr = (f >> 16) & 0xff;
                let fg_g = (f >> 8) & 0xff;
                let fb = f & 0xff;
                let br = (b >> 16) & 0xff;
                let bg_g = (b >> 8) & 0xff;
                let bb = b & 0xff;
                (
                    ((fr * af + br * inv) / 255) as u8,
                    ((fg_g * af + bg_g * inv) / 255) as u8,
                    ((fb * af + bb * inv) / 255) as u8,
                )
            };
            dst[0] = r;
            dst[1] = g;
            dst[2] = bl;
            dst[3] = 0xff;
        }
    }

    pub fn present(&self) {
        if let Err(e) = self.pixels.render() {
            eprintln!("[Surface] render failed: {}", e);
        }
    }
}
