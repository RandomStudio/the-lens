use std::sync::Arc;
use pixels::wgpu;
use pixels::{Pixels, PixelsBuilder, ScalingMode, SurfaceTexture};
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
    let mut pixels = PixelsBuilder::new(bw, bh, surface_texture)
        .texture_format(wgpu::TextureFormat::Rgba8Unorm)
        .request_adapter_options(wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .build()
        .expect("failed to create Pixels");
    // Default is `PixelPerfect` (integer-multiple scaling, letterboxes when surface
    // size isn't an exact multiple of the buffer). Use `Fill` so the buffer scales
    // smoothly to fill the surface while preserving aspect ratio.
    pixels.set_scaling_mode(ScalingMode::Fill);
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

    /// Blit `src` into the GPU frame buffer.
    ///
    /// Pixels must be packed `0xAA_BB_GG_RR` so their little-endian byte order
    /// (`[R, G, B, A]`) matches the surface's `Rgba8Unorm` format. That makes
    /// the upload a straight `copy_from_slice` of the underlying bytes.
    pub fn write_rgba(&mut self, src: &[u32]) {
        let frame = self.pixels.frame_mut();
        let n = src.len().min(frame.len() / 4);
        let src_bytes = unsafe {
            std::slice::from_raw_parts(src.as_ptr() as *const u8, n * 4)
        };
        frame[..n * 4].copy_from_slice(src_bytes);
    }

    pub fn present(&self) {
        if let Err(e) = self.pixels.render() {
            eprintln!("[Surface] render failed: {}", e);
        }
    }
}
