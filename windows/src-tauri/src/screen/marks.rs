//! Round 4C item 5: the marks a brain asks to draw, checked and mapped (pure).
//!
//! Marks come in the last picture's pixels; each is clamped to the picture,
//! one that can't be placed is dropped (and counted, so he can say so), and
//! the rest are mapped to desktop pixels with the picture's scale and origin.
//! Marks are data only: shapes and short text, never markup.

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Circle,
    Box,
    Underline,
    Arrow,
    Highlight,
    Write,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Colour {
    Red,
    Green,
    Yellow,
    Blue,
}

impl Colour {
    /// 0x00BBGGRR, as GDI takes it.
    pub fn colorref(self) -> u32 {
        match self {
            Colour::Red => 0x003C3CFF,
            Colour::Green => 0x0084DC3D,
            Colour::Yellow => 0x0000D7FF,
            Colour::Blue => 0x00FF9A3C,
        }
    }
}

/// One mark in desktop pixels. For an arrow, (x, y) is the start and (x2, y2) the tip;
/// for the others (x, y)–(x2, y2) is the box.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub shape: Shape,
    pub colour: Colour,
    pub x: i32,
    pub y: i32,
    pub x2: i32,
    pub y2: i32,
    /// Handwriting (at most 40 characters) or a label (at most 40).
    pub text: String,
}

pub const MAX_MARKS: usize = 12;
pub const MAX_TEXT: usize = 40;

fn short(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(MAX_TEXT).collect()
}

/// The marks of one `draw_on_screen` call, and how many were dropped.
/// `frame` is (scale, origin, picture width, picture height) of the last look.
pub fn parse(input: &Value, frame: (f64, (i32, i32), u32, u32)) -> (Vec<Mark>, usize) {
    let (scale, (ox, oy), pw, ph) = frame;
    let (pw, ph) = (pw as f64, ph as f64);
    let to_desk = |v: f64, o: i32| o + (v / scale).round() as i32;
    let mut out = Vec::new();
    let mut dropped = 0;
    for m in input["marks"].as_array().map(Vec::as_slice).unwrap_or(&[]).iter().take(MAX_MARKS) {
        let shape = match m["shape"].as_str().unwrap_or("") {
            "circle" => Shape::Circle,
            "box" => Shape::Box,
            "underline" => Shape::Underline,
            "arrow" => Shape::Arrow,
            "highlight" => Shape::Highlight,
            "write" => Shape::Write,
            _ => {
                dropped += 1;
                continue;
            }
        };
        let colour = match m["color"].as_str().unwrap_or("red") {
            "green" => Colour::Green,
            "yellow" => Colour::Yellow,
            "blue" => Colour::Blue,
            _ => Colour::Red,
        };
        let (Some(x), Some(y)) = (m["x"].as_f64(), m["y"].as_f64()) else {
            dropped += 1;
            continue;
        };
        if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 || x > pw || y > ph {
            dropped += 1;
            continue;
        }
        let w = m["w"].as_f64().filter(|v| v.is_finite()).unwrap_or(0.0);
        let h = m["h"].as_f64().filter(|v| v.is_finite()).unwrap_or(0.0);
        let text = short(m["text"].as_str().or(m["label"].as_str()).unwrap_or(""));
        let (x2, y2) = match shape {
            // An arrow's size says where it ends and may be negative.
            Shape::Arrow => ((x + w).clamp(0.0, pw), (y + h).clamp(0.0, ph)),
            Shape::Write => {
                if text.is_empty() {
                    dropped += 1;
                    continue;
                }
                let size = if h > 0.0 { h.min(80.0) } else { 24.0 };
                ((x + size * 0.6 * text.chars().count() as f64).min(pw), (y + size).min(ph))
            }
            _ => {
                if w <= 0.0 || h <= 0.0 {
                    dropped += 1;
                    continue;
                }
                ((x + w).min(pw), (y + h).min(ph))
            }
        };
        out.push(Mark { shape, colour, x: to_desk(x, ox), y: to_desk(y, oy), x2: to_desk(x2, ox), y2: to_desk(y2, oy), text });
    }
    (out, dropped)
}

/// Where Groovy points for a mark (desktop pixels): an arrow's tip, else its middle.
pub fn point_of(m: &Mark) -> (i32, i32) {
    if m.shape == Shape::Arrow {
        (m.x2, m.y2)
    } else {
        ((m.x + m.x2) / 2, (m.y + m.y2) / 2)
    }
}

/// The desktop area the marks need (with a margin), to size the overlay.
pub fn bounds(marks: &[Mark]) -> Option<(i32, i32, i32, i32)> {
    let mut it = marks.iter();
    let first = it.next()?;
    let mut b = (first.x.min(first.x2), first.y.min(first.y2), first.x.max(first.x2), first.y.max(first.y2));
    for m in it {
        b = (b.0.min(m.x.min(m.x2)), b.1.min(m.y.min(m.y2)), b.2.max(m.x.max(m.x2)), b.3.max(m.y.max(m.y2)));
    }
    Some((b.0 - 60, b.1 - 60, b.2 + 60 + 400, b.3 + 60))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn marks_map_back_to_the_screen_at_each_scale_and_on_a_second_monitor() {
        // 100 %: a 1568-wide picture of a 1568-wide monitor.
        let (m, d) = parse(&json!({ "marks": [{ "shape": "box", "x": 100, "y": 50, "w": 20, "h": 10 }] }), (1.0, (0, 0), 1568, 882));
        assert_eq!(d, 0);
        assert_eq!((m[0].x, m[0].y, m[0].x2, m[0].y2), (100, 50, 120, 60));
        // A 4K monitor at 200 % (3840 physical px), picture scale 0.3729.
        let s = 1430.0 / 3840.0;
        let (m, _) = parse(&json!({ "marks": [{ "shape": "circle", "x": 715, "y": 402, "w": 10, "h": 10, "color": "blue" }] }), (s, (0, 0), 1430, 804));
        assert!((m[0].x - 1920).abs() <= 3 && (m[0].y - 1080).abs() <= 3, "{:?}", m[0]);
        assert_eq!(m[0].colour, Colour::Blue);
        // 150 %, a second monitor to the left (negative origin).
        let s = 1430.0 / 2880.0;
        let (m, _) = parse(&json!({ "marks": [{ "shape": "underline", "x": 0, "y": 0, "w": 100, "h": 4 }] }), (s, (-2880, 0), 1430, 804));
        assert_eq!((m[0].x, m[0].y), (-2880, 0));
        assert!((m[0].x2 - (-2880 + 201)).abs() <= 1);
    }

    #[test]
    fn marks_are_clamped_and_the_unplaceable_dropped() {
        let frame = (1.0, (0, 0), 800, 600);
        let (m, d) = parse(
            &json!({ "marks": [
                { "shape": "arrow", "x": 700, "y": 500, "w": 400, "h": -900 },
                { "shape": "box", "x": 900, "y": 10, "w": 5, "h": 5 },
                { "shape": "sparkle", "x": 1, "y": 1 },
                { "shape": "write", "x": 10, "y": 10, "h": 20, "text": "Click here then wait for the green tick to appear" },
                { "shape": "box", "x": 790, "y": 590, "w": 50, "h": 50 }
            ] }),
            frame,
        );
        assert_eq!(d, 2, "off the picture, and an unknown shape");
        assert_eq!((m[0].x2, m[0].y2), (800, 0), "the arrow's tip is clamped");
        assert_eq!(m[1].text.chars().count(), MAX_TEXT);
        assert_eq!((m[2].x2, m[2].y2), (800, 600));
        assert!(parse(&json!({}), frame).0.is_empty());
    }
}
