//! Round 4C item 2: a screen picture made small enough to send (pure, no OS calls).
//!
//! The long side is at most 1568 px and the picture at most 1.15 megapixels, then
//! JPEG at quality 80 (lower only if it is still over 400 KB). The full bitmap is
//! the caller's and is freed by it before the picture goes anywhere.

pub const MAX_SIDE: u32 = 1568;
pub const MAX_PIXELS: f64 = 1_150_000.0;
pub const MAX_BYTES: usize = 400 * 1024;

/// One picture of the screen, in memory only.
#[derive(Clone)]
pub struct Shot {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Picture pixels per screen pixel (1.0 or less).
    pub scale: f64,
    /// The captured area's top-left corner on the desktop, in physical pixels.
    pub origin: (i32, i32),
    /// Where the pointer was, in the picture's pixels (None when outside it).
    pub pointer: Option<(i32, i32)>,
    /// 32×32 grey levels, to tell whether the screen changed (keep watching).
    pub thumb: Vec<u8>,
}

/// The picture's size for a `w`×`h` capture, and the scale used.
pub fn target_size(w: u32, h: u32) -> (u32, u32, f64) {
    if w == 0 || h == 0 {
        return (0, 0, 1.0);
    }
    let by_side = MAX_SIDE as f64 / w.max(h) as f64;
    let by_area = (MAX_PIXELS / (w as f64 * h as f64)).sqrt();
    let scale = by_side.min(by_area).min(1.0);
    let tw = ((w as f64 * scale).floor() as u32).max(1);
    let th = ((h as f64 * scale).floor() as u32).max(1);
    (tw, th, scale)
}

/// A BGRA bitmap (rows `stride` bytes apart) averaged down to `tw`×`th` RGB.
pub fn shrink_bgra(src: &[u8], w: u32, h: u32, stride: usize, tw: u32, th: u32) -> Vec<u8> {
    let mut out = vec![0u8; (tw * th * 3) as usize];
    let (w, h, tw, th) = (w as usize, h as usize, tw as usize, th as usize);
    // The source columns each output column averages, worked out once.
    let cols: Vec<(usize, usize)> = (0..tw).map(|x| (x * w / tw, ((x + 1) * w / tw).max(x * w / tw + 1).min(w))).collect();
    let mut acc = vec![[0u32; 3]; tw];
    for y in 0..th {
        let (y0, y1) = (y * h / th, ((y + 1) * h / th).max(y * h / th + 1).min(h));
        acc.iter_mut().for_each(|a| *a = [0; 3]);
        for sy in y0..y1 {
            let row = &src[sy * stride..sy * stride + w * 4];
            for (x, &(x0, x1)) in cols.iter().enumerate() {
                let a = &mut acc[x];
                for p in row[x0 * 4..x1 * 4].chunks_exact(4) {
                    a[0] += p[2] as u32;
                    a[1] += p[1] as u32;
                    a[2] += p[0] as u32;
                }
            }
        }
        for (x, &(x0, x1)) in cols.iter().enumerate() {
            let n = ((x1 - x0) * (y1 - y0)) as u32;
            let o = (y * tw + x) * 3;
            for c in 0..3 {
                out[o + c] = ((acc[x][c] + n / 2) / n) as u8;
            }
        }
    }
    out
}

/// RGB to JPEG: quality 80, then 65 and 50 if it is still over 400 KB.
pub fn encode(rgb: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let mut last = Vec::new();
    for q in [80u8, 65, 50] {
        let mut buf = Vec::with_capacity(MAX_BYTES / 2);
        jpeg_encoder::Encoder::new(&mut buf, q)
            .encode(rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb)
            .map_err(|e| format!("couldn't make the picture: {e}"))?;
        if buf.len() <= MAX_BYTES {
            return Ok(buf);
        }
        last = buf;
    }
    Ok(last)
}

/// THUMB×THUMB grey levels of an RGB picture (box averages).
pub const THUMB: u32 = 32;

pub fn thumb(rgb: &[u8], w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (THUMB * THUMB) as usize];
    for ty in 0..THUMB {
        let (y0, y1) = (ty * h / THUMB, ((ty + 1) * h / THUMB).max(ty * h / THUMB + 1).min(h));
        for tx in 0..THUMB {
            let (x0, x1) = (tx * w / THUMB, ((tx + 1) * w / THUMB).max(tx * w / THUMB + 1).min(w));
            let (mut sum, mut n) = (0u64, 0u64);
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = ((y * w + x) * 3) as usize;
                    if i + 2 < rgb.len() {
                        sum += (rgb[i] as u64 * 3 + rgb[i + 1] as u64 * 6 + rgb[i + 2] as u64) / 10;
                        n += 1;
                    }
                }
            }
            out[(ty * THUMB + tx) as usize] = sum.checked_div(n).unwrap_or(0) as u8;
        }
    }
    out
}

/// Whether two thumbnails show a changed screen: at least 3 of the 1024 cells
/// moved by more than 12 levels (a blinking caret or a ticking clock is one or two).
pub fn changed(a: &[u8], b: &[u8]) -> bool {
    a.len() != b.len() || a.iter().zip(b).filter(|(x, y)| x.abs_diff(**y) > 12).count() >= 3
}

/// The whole step: a BGRA capture of the area at `origin` into a small JPEG.
/// `pointer` is in desktop pixels.
pub fn shot_from_bgra(src: &[u8], w: u32, h: u32, stride: usize, origin: (i32, i32), pointer: Option<(i32, i32)>) -> Result<Shot, String> {
    let (tw, th, scale) = target_size(w, h);
    if tw == 0 {
        return Err("the screen area is empty".into());
    }
    let rgb = shrink_bgra(src, w, h, stride, tw, th);
    let jpeg = encode(&rgb, tw, th)?;
    let pointer = pointer.and_then(|(px, py)| {
        let (x, y) = (((px - origin.0) as f64 * scale) as i32, ((py - origin.1) as f64 * scale) as i32);
        (x >= 0 && y >= 0 && x < tw as i32 && y < th as i32).then_some((x, y))
    });
    let thumb = thumb(&rgb, tw, th);
    Ok(Shot { jpeg, width: tw, height: th, scale, origin, pointer, thumb })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_4k_monitor_fits_both_limits() {
        let (w, h, s) = target_size(3840, 2160);
        assert!(w <= MAX_SIDE && h <= MAX_SIDE);
        assert!((w as f64 * h as f64) <= MAX_PIXELS);
        assert!(s < 0.4 && s > 0.3, "{s}");
        // A small screen is left as it is.
        assert_eq!(target_size(1024, 768), (1024, 768, 1.0));
        // A very wide one is held by its long side.
        let (w, _, _) = target_size(5120, 1000);
        assert!(w <= MAX_SIDE);
    }

    #[test]
    fn shrinking_keeps_the_colour_and_the_pointer() {
        let (w, h) = (3840u32, 2160u32);
        // Magenta in BGRA.
        let src: Vec<u8> = std::iter::repeat([255u8, 0, 255, 255]).take((w * h) as usize).flatten().collect();
        let t = std::time::Instant::now();
        let shot = shot_from_bgra(&src, w, h, w as usize * 4, (100, 50), Some((100 + 1920, 50 + 1080))).unwrap();
        eprintln!("4K magenta: {} bytes in {:?}", shot.jpeg.len(), t.elapsed());
        assert!(shot.jpeg.len() <= MAX_BYTES);
        assert_eq!(&shot.jpeg[..2], &[0xFF, 0xD8]);
        let (px, py) = shot.pointer.unwrap();
        assert!((px - shot.width as i32 / 2).abs() <= 1 && (py - shot.height as i32 / 2).abs() <= 1);
        let rgb = shrink_bgra(&src[..64 * 4 * 2], 64, 2, 64 * 4, 8, 1);
        assert_eq!(&rgb[..3], &[255, 0, 255]);
        // A pointer on another monitor isn't in the picture.
        assert!(shot_from_bgra(&src, w, h, w as usize * 4, (0, 0), Some((-5, 10))).unwrap().pointer.is_none());
    }

    #[test]
    fn a_changed_screen_is_told_from_a_blinking_caret() {
        let (w, h) = (640u32, 360u32);
        let mut rgb = vec![200u8; (w * h * 3) as usize];
        let a = thumb(&rgb, w, h);
        assert_eq!(a.len(), 1024);
        assert!(!changed(&a, &a));
        // A caret: a few pixels in one cell.
        for y in 10..22 {
            let i = ((y * w + 30) * 3) as usize;
            rgb[i..i + 6].fill(0);
        }
        assert!(!changed(&a, &thumb(&rgb, w, h)), "a caret is not a change");
        // A window opened over a quarter of the screen.
        for y in 0..h / 2 {
            for x in 0..w / 2 {
                let i = ((y * w + x) * 3) as usize;
                rgb[i..i + 3].fill(20);
            }
        }
        assert!(changed(&a, &thumb(&rgb, w, h)));
    }

    #[test]
    fn a_busy_4k_screen_stays_under_400_kb() {
        let (w, h) = (3840u32, 2160u32);
        // Noise-like content is the worst case for JPEG.
        let mut seed = 7u32;
        let src: Vec<u8> = (0..w * h * 4)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 24) as u8
            })
            .collect();
        let shot = shot_from_bgra(&src, w, h, w as usize * 4, (0, 0), None).unwrap();
        assert!(shot.jpeg.len() <= MAX_BYTES, "{}", shot.jpeg.len());
    }
}
