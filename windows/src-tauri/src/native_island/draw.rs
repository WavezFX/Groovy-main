// The native island's drawing (prototype, see docs/plan/NATIVE_ISLAND.md).
//
// A few hundred lines of software rasterising instead of a browser: the pill,
// Groovy's eyes and their glow, drawn with anti-aliased signed distances into
// a premultiplied BGRA buffer that Windows shows with UpdateLayeredWindow.
// Text arrives as a coverage mask (GDI draws it on Windows). No platform code
// here, so it is tested on every OS.

// Only the Windows window draws with it; elsewhere only the tests do.
#![cfg_attr(not(windows), allow(dead_code))]

/// Premultiplied colour, 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba(pub f32, pub f32, pub f32, pub f32);

impl Rgba {
    pub const fn rgb8(r: u8, g: u8, b: u8, a: f32) -> Rgba {
        Rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
    }
}

/// The colours of the look he wears (the island's eyes: `eyeInk`, `eyeGlow`
/// in src/mochi/skins.ts).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub eye: Rgba,
    pub glow: Rgba,
}

pub fn palette(look: &str) -> Palette {
    match look {
        "ember" => Palette { eye: Rgba::rgb8(255, 190, 120, 1.0), glow: Rgba::rgb8(255, 120, 40, 0.95) },
        "frost" => Palette { eye: Rgba::rgb8(150, 220, 255, 1.0), glow: Rgba::rgb8(70, 170, 255, 0.95) },
        // Night, the default, and anything unknown.
        _ => Palette { eye: Rgba::rgb8(150, 255, 185, 1.0), glow: Rgba::rgb8(80, 255, 140, 0.95) },
    }
}

/// A coverage mask (text), one byte a pixel.
pub struct Mask {
    pub w: u32,
    pub h: u32,
    pub a: Vec<u8>,
}

/// Premultiplied BGRA pixels, as `UpdateLayeredWindow` takes them.
pub struct Canvas {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Canvas {
        Canvas { w, h, px: vec![0; (w * h * 4) as usize] }
    }

    pub fn clear(&mut self) {
        self.px.iter_mut().for_each(|p| *p = 0);
    }

    /// Alpha (0..255) at a pixel.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn alpha(&self, x: u32, y: u32) -> u8 {
        self.px[((y * self.w + x) * 4 + 3) as usize]
    }

    /// Source-over of `c` at `coverage` (0..1) onto one pixel.
    fn blend(&mut self, x: u32, y: u32, c: Rgba, coverage: f32) {
        let k = (coverage * c.3).clamp(0.0, 1.0);
        if k <= 0.0 {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        let src = [c.2 * k, c.1 * k, c.0 * k, k];
        for (n, s) in src.iter().enumerate() {
            let d = self.px[i + n] as f32 / 255.0;
            self.px[i + n] = ((s + d * (1.0 - k)) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
    }

    /// Paints `c` wherever `coverage(x + 0.5, y + 0.5)` is above zero, inside a box.
    fn paint(&mut self, bx: (f32, f32, f32, f32), c: Rgba, coverage: impl Fn(f32, f32) -> f32) {
        let x0 = bx.0.floor().max(0.0) as u32;
        let y0 = bx.1.floor().max(0.0) as u32;
        let x1 = (bx.2.ceil().max(0.0) as u32).min(self.w);
        let y1 = (bx.3.ceil().max(0.0) as u32).min(self.h);
        for y in y0..y1 {
            for x in x0..x1 {
                let a = coverage(x as f32 + 0.5, y as f32 + 0.5);
                if a > 0.0 {
                    self.blend(x, y, c, a.min(1.0));
                }
            }
        }
    }

    /// A rounded rectangle, its top corners `rt` and bottom corners `rb` round.
    pub fn rounded_rect(&mut self, r: Rect, rt: f32, rb: f32, c: Rgba) {
        self.paint((r.x - 1.0, r.y - 1.0, r.x + r.w + 1.0, r.y + r.h + 1.0), c, |px, py| (0.5 - rect_sdf(r, rt, rb, px, py)).clamp(0.0, 1.0));
    }

    /// A one-pixel line just inside the rounded rectangle's edge.
    pub fn rounded_rim(&mut self, r: Rect, rt: f32, rb: f32, c: Rgba) {
        self.paint((r.x - 1.0, r.y - 1.0, r.x + r.w + 1.0, r.y + r.h + 1.0), c, |px, py| {
            let d = rect_sdf(r, rt, rb, px, py);
            (0.5 - d).clamp(0.0, 1.0) - (0.5 - (d + 1.0)).clamp(0.0, 1.0)
        });
    }

    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgba) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        self.paint((cx - rx - 1.0, cy - ry - 1.0, cx + rx + 1.0, cy + ry + 1.0), c, |px, py| {
            let (nx, ny) = ((px - cx) / rx, (py - cy) / ry);
            let d = ((nx * nx + ny * ny).sqrt() - 1.0) * rx.min(ry);
            (0.5 - d).clamp(0.0, 1.0)
        });
    }

    /// A soft glow: full at the centre, gone at `radius`, kept inside `clip`.
    pub fn glow(&mut self, cx: f32, cy: f32, radius: f32, c: Rgba, clip: Rect) {
        let bx = (cx - radius, cy - radius, cx + radius, cy + radius);
        let bx = (bx.0.max(clip.x), bx.1.max(clip.y), bx.2.min(clip.x + clip.w), bx.3.min(clip.y + clip.h));
        self.paint(bx, c, |px, py| {
            let t = 1.0 - (((px - cx).powi(2) + (py - cy).powi(2)).sqrt() / radius);
            if t <= 0.0 { 0.0 } else { t * t }
        });
    }

    /// Text (or any mask) in colour `c`, its top left at (`x`, `y`).
    pub fn mask(&mut self, m: &Mask, x: i32, y: i32, c: Rgba) {
        for my in 0..m.h {
            for mx in 0..m.w {
                let (px, py) = (x + mx as i32, y + my as i32);
                if px < 0 || py < 0 || px as u32 >= self.w || py as u32 >= self.h {
                    continue;
                }
                let a = m.a[(my * m.w + mx) as usize];
                if a > 0 {
                    self.blend(px as u32, py as u32, c, a as f32 / 255.0);
                }
            }
        }
    }

    /// The faintest alpha a layered window still takes the mouse on: an
    /// invisible hover band around the folded pill.
    pub fn hit_band(&mut self, r: Rect) {
        let x0 = r.x.max(0.0) as u32;
        let y0 = r.y.max(0.0) as u32;
        let x1 = ((r.x + r.w) as u32).min(self.w);
        let y1 = ((r.y + r.h) as u32).min(self.h);
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * self.w + x) * 4 + 3) as usize;
                if self.px[i] == 0 {
                    self.px[i] = 1;
                }
            }
        }
    }
}

/// Signed distance to a rounded rectangle (negative inside).
fn rect_sdf(r: Rect, rt: f32, rb: f32, px: f32, py: f32) -> f32 {
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let rad = if py < cy { rt } else { rb }.min(r.w / 2.0).min(r.h / 2.0);
    let qx = (px - cx).abs() - (r.w / 2.0 - rad);
    let qy = (py - cy).abs() - (r.h / 2.0 - rad);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - rad
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// Window and pill sizes, in logical pixels (times the display scale).
pub const WIN_W: f32 = 320.0;
pub const WIN_H: f32 = 48.0;
const FOLDED: (f32, f32) = (176.0, 30.0);
const OPEN: (f32, f32) = (300.0, 40.0);
/// The invisible band around the folded pill that still counts as hovering it.
const HOVER_MARGIN: f32 = 8.0;

/// Where everything goes for an openness `open` (0 folded … 1 open), at `scale`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub pill: Rect,
    pub hover: Rect,
    /// The centres of the two eyes, their half width and half height (open).
    pub eyes: [(f32, f32); 2],
    pub eye_r: (f32, f32),
    /// Where the text starts (left, vertical centre) and the room it has.
    pub text: (f32, f32, f32),
    pub font_px: f32,
}

pub fn layout(open: f32, scale: f32) -> Layout {
    let o = open.clamp(0.0, 1.15);
    let w = (FOLDED.0 + (OPEN.0 - FOLDED.0) * o) * scale;
    let h = (FOLDED.1 + (OPEN.1 - FOLDED.1) * o.min(1.0)) * scale;
    let pill = Rect { x: (WIN_W * scale - w) / 2.0, y: 0.0, w, h };
    let m = HOVER_MARGIN * scale;
    let hover = Rect { x: pill.x - m, y: 0.0, w: pill.w + 2.0 * m, h: pill.h + m };
    let cy = pill.h / 2.0;
    let eye_r = (2.6 * scale * (1.0 + 0.2 * o.min(1.0)), 4.6 * scale * (1.0 + 0.2 * o.min(1.0)));
    let ex = pill.x + h * 0.62;
    let gap = 5.6 * scale * (1.0 + 0.2 * o.min(1.0));
    let eyes = [(ex - gap, cy), (ex + gap, cy)];
    let text_x = ex + gap + eye_r.0 + 12.0 * scale;
    let font_px = (13.0 + 1.0 * o.min(1.0)) * scale;
    Layout { pill, hover, eyes, eye_r, text: (text_x, cy, (pill.x + pill.w - h * 0.5 - text_x).max(0.0)), font_px }
}

/// Everything one frame shows.
pub struct Scene<'a> {
    pub palette: Palette,
    pub open: f32,
    /// 0 eyes open … 1 shut.
    pub blink: f32,
    pub scale: f32,
    pub text: Option<&'a Mask>,
}

const GLASS: Rgba = Rgba(0.07, 0.075, 0.085, 0.86);
const RIM: Rgba = Rgba(1.0, 1.0, 1.0, 0.12);
const INK: Rgba = Rgba(0.93, 0.95, 0.96, 0.92);

pub fn draw(c: &mut Canvas, s: &Scene) -> Layout {
    c.clear();
    let l = layout(s.open, s.scale);
    let p = l.pill;
    let (rt, rb) = (6.0 * s.scale, p.h / 2.0);
    c.rounded_rect(p, rt, rb, GLASS);
    c.rounded_rim(p, rt, rb, RIM);
    // His glow first, then the eyes over it.
    let glow = Rgba(s.palette.glow.0, s.palette.glow.1, s.palette.glow.2, s.palette.glow.3 * 0.42);
    let inside = Rect { x: p.x + 1.0, y: p.y + 1.0, w: p.w - 2.0, h: p.h - 2.0 };
    for (x, y) in l.eyes {
        c.glow(x, y, l.eye_r.1 * 2.6, glow, inside);
    }
    let shut = s.blink.clamp(0.0, 1.0);
    let ry = (l.eye_r.1 * (1.0 - shut)).max(l.eye_r.1 * 0.14);
    for (x, y) in l.eyes {
        c.ellipse(x, y, l.eye_r.0, ry, s.palette.eye);
    }
    if let Some(m) = s.text {
        let y = (l.text.1 - m.h as f32 / 2.0).round() as i32;
        c.mask(m, l.text.0.round() as i32, y, INK);
    }
    if s.open < 0.05 {
        c.hit_band(l.hover);
    }
    l
}

/// Opening, folding and blinking over time. Between them nothing moves, and
/// `moving` is false: the window then draws no frames at all.
#[derive(Clone, Debug)]
pub struct Anim {
    from: f32,
    to: f32,
    t0: f64,
    blink_t0: Option<f64>,
}

pub const OPEN_S: f64 = 0.24;
pub const BLINK_S: f64 = 0.16;

impl Default for Anim {
    fn default() -> Self {
        Anim { from: 0.0, to: 0.0, t0: f64::MIN, blink_t0: None }
    }
}

impl Anim {
    pub fn target(&self) -> f32 {
        self.to
    }

    /// Open (true) or fold (false) from wherever it is now.
    pub fn set_open(&mut self, now: f64, open: bool) {
        let to = if open { 1.0 } else { 0.0 };
        if to == self.to {
            return;
        }
        self.from = self.open(now);
        self.to = to;
        self.t0 = now;
    }

    pub fn blink(&mut self, now: f64) {
        self.blink_t0 = Some(now);
    }

    pub fn open(&self, now: f64) -> f32 {
        let t = ((now - self.t0) / OPEN_S).clamp(0.0, 1.0) as f32;
        let k = if self.to > self.from {
            // Out with a little overshoot.
            let u = t - 1.0;
            1.0 + 2.2 * u * u * u + 1.2 * u * u
        } else {
            1.0 - (1.0 - t).powi(3)
        };
        self.from + (self.to - self.from) * k
    }

    pub fn shut(&self, now: f64) -> f32 {
        match self.blink_t0 {
            Some(t0) if now >= t0 && now - t0 < BLINK_S => ((now - t0) / BLINK_S * std::f64::consts::PI).sin() as f32,
            _ => 0.0,
        }
    }

    pub fn moving(&self, now: f64) -> bool {
        now - self.t0 < OPEN_S || self.blink_t0.is_some_and(|t0| now - t0 < BLINK_S)
    }
}

/// "14:05" from the hour and minute.
pub fn clock(hour: u32, minute: u32) -> String {
    format!("{:02}:{:02}", hour % 24, minute % 60)
}

/// What the pill says: the time folded, the time and his name open.
pub fn label(hour: u32, minute: u32, name: &str, open: bool) -> String {
    let t = clock(hour, minute);
    let name = name.trim();
    if open && !name.is_empty() {
        let short: String = name.chars().take(18).collect();
        format!("{t}  ·  {short}")
    } else {
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(open: f32, blink: f32) -> (Canvas, Layout) {
        let mut c = Canvas::new(WIN_W as u32, WIN_H as u32);
        let l = draw(&mut c, &Scene { palette: palette("night"), open, blink, scale: 1.0, text: None });
        (c, l)
    }

    #[test]
    fn the_pill_is_drawn_at_the_top_and_the_rest_is_see_through() {
        let (c, l) = scene(0.0, 0.0);
        let mid = (l.pill.x + l.pill.w / 2.0) as u32;
        assert!(c.alpha(mid, 2) > 200, "glass at the top centre");
        assert_eq!(c.alpha(2, 2), 0, "the corner of the window is click-through");
        assert_eq!(c.alpha(mid, (WIN_H - 1.0) as u32), 0, "below the hover band nothing");
        // The hover band is there but invisible.
        let band_x = (l.pill.x - 4.0) as u32;
        assert_eq!(c.alpha(band_x, 4), 1);
    }

    #[test]
    fn the_eyes_wear_the_looks_colour_and_close_in_a_blink() {
        let (open, l) = scene(0.0, 0.0);
        let (x, y) = (l.eyes[0].0 as u32, (l.eyes[0].1 - l.eye_r.1 * 0.7) as u32);
        let i = ((y * open.w + x) * 4) as usize;
        let (b, g, r) = (open.px[i], open.px[i + 1], open.px[i + 2]);
        assert!(g > r && g > b, "Night's eyes are green: {r} {g} {b}");
        let (shut, _) = scene(0.0, 1.0);
        let j = ((y * shut.w + x) * 4) as usize;
        assert!(shut.px[j + 1] < g / 2, "shut, the top of the eye is glass again");
        assert_ne!(palette("ember"), palette("night"));
        assert_eq!(palette("anything"), palette("night"));
    }

    #[test]
    fn hovering_opens_the_pill_wider_and_folding_brings_it_back() {
        let folded = layout(0.0, 1.0);
        let open = layout(1.0, 1.0);
        assert!(open.pill.w > folded.pill.w + 100.0 && open.pill.h > folded.pill.h);
        assert!(folded.hover.contains(folded.pill.x - 4.0, 2.0) && !folded.pill.contains(folded.pill.x - 4.0, 2.0));
        let wide = layout(1.15, 2.0);
        assert!(wide.pill.x >= 0.0 && wide.pill.x + wide.pill.w <= WIN_W * 2.0, "the overshoot stays in the window");
    }

    #[test]
    fn nothing_moves_when_folded_and_idle() {
        let mut a = Anim::default();
        assert!(!a.moving(10.0), "idle: no frames");
        a.set_open(10.0, true);
        assert!(a.moving(10.1));
        assert!(!a.moving(10.0 + OPEN_S + 0.01));
        assert!((a.open(11.0) - 1.0).abs() < 1e-6);
        assert!(a.open(10.0 + OPEN_S * 0.7) > 1.0, "a little overshoot on the way out");
        a.set_open(11.0, false);
        assert!(a.open(11.0 + OPEN_S).abs() < 1e-6);
        a.blink(20.0);
        assert!(a.moving(20.05) && a.shut(20.08) > 0.9);
        assert_eq!(a.shut(20.0 + BLINK_S), 0.0);
        assert!(!a.moving(20.0 + BLINK_S + 0.01));
    }

    #[test]
    fn text_is_composited_in_the_ink_colour() {
        let mut c = Canvas::new(WIN_W as u32, WIN_H as u32);
        let m = Mask { w: 4, h: 4, a: vec![255; 16] };
        let l = draw(&mut c, &Scene { palette: palette("frost"), open: 0.0, blink: 0.0, scale: 1.0, text: Some(&m) });
        let (x, y) = (l.text.0 as u32 + 1, l.text.1 as u32);
        let i = ((y * c.w + x) * 4) as usize;
        assert!(c.px[i] > 200 && c.px[i + 1] > 200 && c.px[i + 2] > 200);
    }

    #[test]
    fn the_label_is_the_time_folded_and_adds_his_name_open() {
        assert_eq!(label(9, 5, "Groovy", false), "09:05");
        assert_eq!(label(23, 59, "Groovy", true), "23:59  ·  Groovy");
        assert_eq!(label(7, 0, "  ", true), "07:00");
    }
}
