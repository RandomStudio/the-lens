use minifb::{Key, Window, WindowOptions};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

const CACHE_RADIUS: usize = 50;

pub fn display_bounds(index: usize) -> (isize, isize, usize, usize) {
    #[cfg(target_os = "macos")]
    {
        use core_graphics::display::CGDisplay;
        let ids = CGDisplay::active_displays().expect("failed to list displays");
        let mut displays: Vec<(isize, isize, usize, usize)> = ids
            .iter()
            .map(|&id| {
                let b = CGDisplay::new(id).bounds();
                (b.origin.x as isize, b.origin.y as isize,
                 b.size.width as usize, b.size.height as usize)
            })
            .collect();
        displays.sort_by_key(|&(x, y, _, _)| (x, y));
        assert!(index < displays.len(),
            "display {} requested but only {} available", index, displays.len());
        return displays[index];
    }
    #[cfg(not(target_os = "macos"))]
    panic!("display_bounds not implemented for this platform");
}

#[cfg(target_os = "macos")]
fn make_fullscreen(window: &Window) {
    use objc::{msg_send, sel, sel_impl, runtime::{Object, YES}};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return };

    unsafe {
        let ns_view: *mut Object = h.ns_view.as_ptr() as *mut Object;
        let ns_window: *mut Object = msg_send![ns_view, window];

        // Extend content view to cover the full window including title bar area,
        // then hide the title bar so it doesn't leave a white strip in fullscreen.
        // NSWindowStyleMaskFullSizeContentView = 1 << 15
        let style: usize = msg_send![ns_window, styleMask];
        let () = msg_send![ns_window, setStyleMask: style | (1usize << 15)];
        let () = msg_send![ns_window, setTitlebarAppearsTransparent: YES];
        let () = msg_send![ns_window, setTitleVisibility: 1isize]; // NSWindowTitleHidden

        // Enable and trigger fullscreen.
        // NSWindowCollectionBehaviorFullScreenPrimary = 1 << 7
        let behavior: usize = msg_send![ns_window, collectionBehavior];
        let () = msg_send![ns_window, setCollectionBehavior: behavior | (1usize << 7)];
        let () = msg_send![ns_window, toggleFullScreen: std::ptr::null::<Object>()];
    }
}

pub struct ImageSequence {
    paths: Arc<Vec<PathBuf>>,
    width: usize,
    height: usize,
    blank: Vec<u32>,
    cache: HashMap<usize, Vec<u32>>,
    in_flight: HashSet<usize>,
    result_tx: mpsc::Sender<(usize, Vec<u32>)>,
    result_rx: mpsc::Receiver<(usize, Vec<u32>)>,
    index_transform: fn(usize, usize) -> usize,
}

impl ImageSequence {
    pub fn load(folder: &str) -> Self {
        let path = Path::new(folder);
        let (tx, rx) = mpsc::channel();

        if !path.exists() {
            eprintln!("[WARN] '{}' does not exist — will show blank frames.", folder);
            return Self::blank_instance(tx, rx);
        }

        let mut entries: Vec<_> = std::fs::read_dir(path)
            .expect("failed to read image folder")
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_lowercase();
                n.ends_with(".png") || n.ends_with(".jpg") || n.ends_with(".jpeg")
            })
            .collect();

        entries.sort_by_key(|e| e.file_name());

        if entries.is_empty() {
            eprintln!("[WARN] No images in '{}' — will show blank frames.", folder);
            return Self::blank_instance(tx, rx);
        }

        let paths: Vec<PathBuf> = entries.iter().map(|e| e.path()).collect();
        println!("[INFO] Found {} images in '{}'", paths.len(), folder);

        Self {
            paths: Arc::new(paths),
            width: 0, height: 0, blank: vec![],
            cache: HashMap::new(), in_flight: HashSet::new(),
            result_tx: tx, result_rx: rx,
            index_transform: |idx, _n| idx,
        }
    }

    pub fn empty() -> Self {
        let (tx, rx) = mpsc::channel();
        Self::blank_instance(tx, rx)
    }

    fn blank_instance(
        tx: mpsc::Sender<(usize, Vec<u32>)>,
        rx: mpsc::Receiver<(usize, Vec<u32>)>,
    ) -> Self {
        Self {
            paths: Arc::new(vec![]),
            width: 0, height: 0, blank: vec![],
            cache: HashMap::new(), in_flight: HashSet::new(),
            result_tx: tx, result_rx: rx,
            index_transform: |idx, _n| idx,
        }
    }

    pub fn set_index_transform(&mut self, f: fn(usize, usize) -> usize) {
        self.index_transform = f;
    }

    pub fn set_dimensions(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.blank = vec![0u32; width * height];
    }

    pub fn frame_count(&self) -> usize {
        self.paths.len()
    }

    pub fn frame_at_angle(&mut self, angle: f64) -> &[u32] {
        if self.paths.is_empty() {
            return &self.blank;
        }

        let n = self.paths.len();
        let idx = ((angle.rem_euclid(360.0) / 360.0) * n as f64) as usize;
        let idx = (self.index_transform)(idx.min(n - 1), n);

        while let Ok((i, frame)) = self.result_rx.try_recv() {
            self.in_flight.remove(&i);
            self.cache.insert(i, frame);
        }

        let window: HashSet<usize> = window_indices(idx, n).collect();
        self.cache.retain(|k, _| window.contains(k));
        self.in_flight.retain(|k| window.contains(k));

        for wi in window_indices(idx, n) {
            if self.cache.contains_key(&wi) { continue; }
            if !self.in_flight.insert(wi) { continue; }
            let paths = Arc::clone(&self.paths);
            let tx = self.result_tx.clone();
            let (w, h) = (self.width, self.height);
            rayon::spawn(move || {
                let frame = decode_frame(&paths[wi], w, h);
                let _ = tx.send((wi, frame));
            });
        }

        if !self.cache.contains_key(&idx) {
            let frame = decode_frame(&self.paths[idx], self.width, self.height);
            self.in_flight.remove(&idx);
            self.cache.insert(idx, frame);
        }

        &self.cache[&idx]
    }
}

fn window_indices(center: usize, n: usize) -> impl Iterator<Item = usize> {
    let n_i = n as isize;
    let c = center as isize;
    let r = CACHE_RADIUS as isize;
    (0..=(2 * r)).map(move |step| {
        let offset = if step == 0 { 0 }
            else if step % 2 == 1 { (step + 1) / 2 }
            else { -(step / 2) };
        ((c + offset).rem_euclid(n_i)) as usize
    })
}

fn decode_frame(path: &Path, width: usize, height: usize) -> Vec<u32> {
    let img = image::open(path)
        .unwrap_or_else(|_| panic!("failed to open {:?}", path))
        .to_rgba8();

    let img_w = img.width() as usize;
    let img_h = img.height() as usize;
    let src_y0 = if img_h > height { (img_h - height) / 2 } else { 0 };
    let dst_y0 = if img_h < height { (height - img_h) / 2 } else { 0 };
    let rows = img_h.min(height);
    let cols = img_w.min(width);

    let mut canvas = vec![0u32; width * height];
    let raw = img.as_raw();
    let stride = img_w * 4;

    for out_row in 0..rows {
        let src = (src_y0 + out_row) * stride;
        let dst = (dst_y0 + out_row) * width;
        for col in 0..cols {
            let p = src + col * 4;
            canvas[dst + col] =
                ((raw[p] as u32) << 16) | ((raw[p + 1] as u32) << 8) | (raw[p + 2] as u32);
        }
    }

    canvas
}

pub struct Viewer {
    window1: Window,
    window2: Window,
    seq1: ImageSequence,
    seq2: ImageSequence,
    w1: (usize, usize),
    w2: (usize, usize),
}

impl Viewer {
    pub fn new(
        mut seq1: ImageSequence,
        display1: usize,
        mut seq2: ImageSequence,
        display2: usize,
    ) -> Self {
        let (x1, y1, w1, h1) = display_bounds(display1);
        let (x2, y2, w2, h2) = display_bounds(display2);

        seq1.set_dimensions(w1, h1);
        seq2.set_dimensions(w2, h2);

        let mut window1 = Window::new("Lens", w1, h1, WindowOptions::default())
            .expect("failed to create window 1");
        window1.set_position(x1, y1);
        window1.set_target_fps(60);

        let mut window2 = Window::new("Remote", w2, h2, WindowOptions::default())
            .expect("failed to create window 2");
        window2.set_position(x2, y2);
        window2.set_target_fps(60);

        #[cfg(target_os = "macos")]
        {
            make_fullscreen(&window1);
            make_fullscreen(&window2);
        }

        Self { window1, window2, seq1, seq2, w1: (w1, h1), w2: (w2, h2) }
    }

    pub fn is_open(&self) -> bool {
        self.window1.is_open()
            && self.window2.is_open()
            && !self.window1.is_key_down(Key::Escape)
            && !self.window2.is_key_down(Key::Escape)
    }

    pub fn render(&mut self, angle: f64) {
        let frame1 = self.seq1.frame_at_angle(angle);
        self.window1
            .update_with_buffer(frame1, self.w1.0, self.w1.1)
            .expect("window 1 update failed");

        let frame2 = self.seq2.frame_at_angle(angle);
        self.window2
            .update_with_buffer(frame2, self.w2.0, self.w2.1)
            .expect("window 2 update failed");
    }
}
