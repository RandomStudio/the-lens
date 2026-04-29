use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

const CACHE_RADIUS: usize = 500;

pub struct ImageSequence {
    paths: Arc<Vec<PathBuf>>,
    width: usize,
    height: usize,
    blank: Vec<u32>,
    cache: HashMap<usize, Vec<u32>>,
    in_flight: HashSet<usize>,
    result_tx: mpsc::Sender<(usize, Vec<u32>)>,
    result_rx: mpsc::Receiver<(usize, Vec<u32>)>,
    index_transform: fn(isize, isize) -> isize,
}

impl ImageSequence {
    pub fn load(folder: &str, index_transform: fn(isize, isize) -> isize) -> Self {
        let path = Path::new(folder);
        let (tx, rx) = mpsc::channel();

        if !path.exists() {
            eprintln!("[ImageSequence] '{}' does not exist — will show blank frames.", folder);
            return Self::blank_instance(tx, rx, index_transform);
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
            eprintln!("[ImageSequence] No images in '{}' — will show blank frames.", folder);
            return Self::blank_instance(tx, rx, index_transform);
        }

        let paths: Vec<PathBuf> = entries.iter().map(|e| e.path()).collect();
        println!("[ImageSequence] Found {} images in '{}'", paths.len(), folder);

        Self {
            paths: Arc::new(paths),
            width: 0, height: 0, blank: vec![],
            cache: HashMap::new(), in_flight: HashSet::new(),
            result_tx: tx, result_rx: rx,
            index_transform,
        }
    }

    fn blank_instance(
        tx: mpsc::Sender<(usize, Vec<u32>)>,
        rx: mpsc::Receiver<(usize, Vec<u32>)>,
        index_transform: fn(isize, isize) -> isize,
    ) -> Self {
        Self {
            paths: Arc::new(vec![]),
            width: 0, height: 0, blank: vec![],
            cache: HashMap::new(), in_flight: HashSet::new(),
            result_tx: tx, result_rx: rx,
            index_transform,
        }
    }

    pub fn set_dimensions(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.blank = vec![0u32; width * height];
        self.cache.clear();
        self.in_flight.clear();
    }

    pub fn frame_count(&self) -> usize {
        self.paths.len()
    }

    pub fn frame_index_at_angle(&self, angle: f64) -> usize {
        if self.paths.is_empty() { return 0; }
        let n = self.paths.len();
        let idx = ((angle.rem_euclid(360.0) / 360.0) * n as f64) as usize;
        (self.index_transform)(idx.min(n - 1) as isize, n as isize)
            .rem_euclid(n as isize) as usize
    }

    pub fn frame_at_angle(&mut self, angle: f64) -> &[u32] {
        if self.paths.is_empty() || self.width == 0 {
            return &self.blank;
        }

        let n = self.paths.len();
        let idx = self.frame_index_at_angle(angle);

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

        if self.cache.contains_key(&idx) {
            return &self.cache[&idx];
        }

        // Cache miss: if a cached frame is within NEAR_TOLERANCE indices of the requested
        // idx, show that one (avoids a sync-decode hit during small lag). Otherwise the
        // gap is large enough that showing it would freeze the display, so sync-decode
        // the requested frame now.
        const NEAR_TOLERANCE: usize = 10;
        if let Some(nearest) = nearest_key(&self.cache, idx, n) {
            let n_i = n as isize;
            let raw_diff = (nearest as isize - idx as isize).abs();
            let dist = raw_diff.min(n_i - raw_diff) as usize;
            if dist <= NEAR_TOLERANCE {
                return &self.cache[&nearest];
            }
        }

        let frame = decode_frame(&self.paths[idx], self.width, self.height);
        self.in_flight.remove(&idx);
        self.cache.insert(idx, frame);
        &self.cache[&idx]
    }

    pub fn peek_dimensions(&self) -> Option<(usize, usize)> {
        let path = self.paths.first()?;
        let img = image::open(path).ok()?;
        Some((img.width() as usize, img.height() as usize))
    }
}

fn nearest_key(cache: &HashMap<usize, Vec<u32>>, idx: usize, n: usize) -> Option<usize> {
    if cache.is_empty() { return None; }
    let n_i = n as isize;
    let target = idx as isize;
    cache.keys().min_by_key(|&&k| {
        let diff = (k as isize - target).abs();
        diff.min(n_i - diff)
    }).copied()
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

/// Decode an image and rescale (cover) into a `width × height` canvas.
///
/// Pixels are packed `0xff_BB_GG_RR` so that on little-endian targets the
/// in-memory byte order is `[R, G, B, A]` — matching `Rgba8Unorm` exactly,
/// which lets the surface blit be a single `copy_from_slice`.
pub fn decode_frame(path: &Path, width: usize, height: usize) -> Vec<u32> {
    let opaque = vec![0xff00_0000u32; width * height];
    let img = match image::ImageReader::open(path)
        .and_then(|r| r.with_guessed_format())
        .and_then(|r| r.decode().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)))
    {
        Ok(img) => img.to_rgb8(),
        Err(e) => {
            eprintln!("[decode_frame] failed to decode {:?}: {}", path, e);
            return opaque;
        }
    };

    let img_w = img.width() as usize;
    let img_h = img.height() as usize;

    let scale = width as f64 / img_w as f64;
    let scaled_h = (img_h as f64 * scale).round() as usize;
    let y_src_offset = if scaled_h > height { (scaled_h - height) / 2 } else { 0 };
    let y_dst_offset = if scaled_h < height { (height - scaled_h) / 2 } else { 0 };
    let visible_rows = scaled_h.min(height);

    let inv_scale = 1.0 / scale;
    let mut canvas = opaque;
    let raw = img.as_raw();

    for oy in 0..visible_rows {
        let scaled_y = oy + y_src_offset;
        let sy = ((scaled_y as f64 * inv_scale) as usize).min(img_h - 1);
        let dst_row = oy + y_dst_offset;
        let row_base = sy * img_w * 3;
        let dst_base = dst_row * width;
        for ox in 0..width {
            let sx = ((ox as f64 * inv_scale) as usize).min(img_w - 1);
            let p = row_base + sx * 3;
            canvas[dst_base + ox] = 0xff00_0000
                | ((raw[p + 2] as u32) << 16)
                | ((raw[p + 1] as u32) << 8)
                | (raw[p] as u32);
        }
    }

    canvas
}

pub fn scale_frame_to(src: &[u32], sw: usize, sh: usize, tw: usize, th: usize) -> Vec<u32> {
    if sw == 0 || sh == 0 || tw == 0 || th == 0 { return vec![0u32; tw * th]; }
    let mut out = vec![0u32; tw * th];
    for dy in 0..th {
        let sy = (dy * sh) / th;
        for dx in 0..tw {
            let sx = (dx * sw) / tw;
            out[dy * tw + dx] = src[(sy * sw + sx).min(src.len() - 1)];
        }
    }
    out
}
