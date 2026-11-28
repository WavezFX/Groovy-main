//! Round 4C item 4: answers about the screen in steps, with marks (pure).
//!
//! A brain answering in the chat or a call has no tools, so it writes its marks
//! inline as tags: `[[arrow 120 300 80 -40 red Save]]` (shape, x, y, w, h in the
//! picture's pixels, then an optional colour and a label, or the text for
//! `write`). A long explanation comes as "Step 1: …", "Step 2: …" lines (up to 8).
//! He says step 1 and draws its marks; "next" says and draws the next one.
//! Tags are never spoken (`speakable`, mirrored in `src/voice/sentences.ts`).

use serde_json::{json, Value};

pub const MAX_STEPS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// The words to say, without tags.
    pub say: String,
    /// The marks, as `draw_on_screen` takes them.
    pub marks: Vec<Value>,
}

const SHAPES: [&str; 6] = ["circle", "box", "underline", "arrow", "highlight", "write"];
const COLOURS: [&str; 4] = ["red", "green", "yellow", "blue"];

/// One tag's inside ("arrow 120 300 80 -40 red Save") as a mark.
fn tag_mark(inner: &str) -> Option<Value> {
    let mut words = inner.split_whitespace();
    let shape = words.next()?.to_lowercase();
    if !SHAPES.contains(&shape.as_str()) {
        return None;
    }
    let mut n = [0f64; 4];
    for v in n.iter_mut() {
        *v = words.next()?.trim_end_matches(',').parse().ok()?;
    }
    let rest: Vec<&str> = words.collect();
    let (colour, label) = match rest.first() {
        Some(c) if COLOURS.contains(&c.to_lowercase().as_str()) => (c.to_lowercase(), rest[1..].join(" ")),
        _ => ("red".to_string(), rest.join(" ")),
    };
    let label = label.trim_matches('"').to_string();
    let mut m = json!({ "shape": shape, "x": n[0], "y": n[1], "w": n[2], "h": n[3], "color": colour });
    if !label.is_empty() {
        m[if shape == "write" { "text" } else { "label" }] = json!(label);
    }
    Some(m)
}

/// The text without its tags, and the marks they held.
fn split_tags(text: &str) -> (String, Vec<Value>) {
    let mut out = String::new();
    let mut marks = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("]]") {
            Some(end) => {
                marks.extend(tag_mark(&rest[start + 2..start + 2 + end]));
                rest = &rest[start + 2 + end + 2..];
            }
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    (tidy(&out), marks)
}

fn tidy(s: &str) -> String {
    let mut t = s.split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
    for p in [" .", " ,", " !", " ?", " :"] {
        t = t.replace(p, &p[1..]);
    }
    t.trim().to_string()
}

/// Where "Step N" starts a line (or the text), as (byte offset, N).
fn step_starts(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let lower = text.to_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("step ") {
        let at = from + i;
        let line_start = at == 0 || lower[..at].trim_end_matches([' ', '*', '#']).ends_with('\n');
        let digits: String = lower[at + 5..].chars().take_while(|c| c.is_ascii_digit()).collect();
        let after = lower[at + 5 + digits.len()..].chars().next();
        if line_start && !digits.is_empty() && matches!(after, Some(':' | '.' | ')' | ' ' | '-')) {
            out.push((at, digits.parse().unwrap_or(0)));
        }
        from = at + 5;
    }
    out
}

/// A reply split into its steps (one step when it has none), each with its
/// marks. None when there is nothing to draw and only one step.
pub fn parse(text: &str) -> Option<Vec<Step>> {
    let starts = step_starts(text);
    let mut pieces: Vec<&str> = Vec::new();
    if starts.len() >= 2 && starts[0].1 == 1 {
        let intro = text[..starts[0].0].trim();
        for (k, &(at, _)) in starts.iter().enumerate() {
            let end = starts.get(k + 1).map(|s| s.0).unwrap_or(text.len());
            pieces.push(&text[at..end]);
        }
        let mut steps: Vec<Step> = pieces
            .iter()
            .take(MAX_STEPS)
            .map(|p| {
                let (say, marks) = split_tags(p);
                Step { say, marks }
            })
            .collect();
        if !intro.is_empty() {
            let (say, marks) = split_tags(intro);
            steps[0].say = format!("{say} {}", steps[0].say).trim().to_string();
            steps[0].marks.splice(0..0, marks);
        }
        return Some(steps);
    }
    let (say, marks) = split_tags(text);
    (!marks.is_empty()).then(|| vec![Step { say, marks }])
}

/// What he says of a reply: up to step 2, with no tags (an unclosed tag at the
/// end of a streaming reply is cut). The page does the same to the stream.
// The page does the speaking; this copy is the tested reference for its rule.
#[cfg_attr(not(test), allow(dead_code))]
pub fn speakable(text: &str) -> String {
    let starts = step_starts(text);
    let cut = match starts.as_slice() {
        [(_, 1), (at, _), ..] => &text[..*at],
        _ => text,
    };
    split_tags(cut).0
}

/// The words for step `k` (0-based) of `n`, with what to say to go on.
pub fn words(step: &Step, k: usize, n: usize) -> String {
    if k + 1 < n {
        format!("{} (Say \"next\" for step {} of {n}.)", step.say, k + 2)
    } else {
        step.say.clone()
    }
}

/// A spoken or typed command about the marks, answered without a brain.
#[derive(Debug, PartialEq, Eq)]
pub enum Said {
    Next,
    Clear,
}

pub fn command(text: &str) -> Option<Said> {
    let t: String = text.to_lowercase().chars().filter(|c| c.is_alphanumeric() || *c == ' ').collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let t = t.trim_start_matches("hey groovy ").trim_start_matches("ok ").trim_start_matches("okay ");
    const NEXT: [&str; 8] = ["next", "next step", "next one", "go on", "whats next", "what next", "and then", "continue"];
    const CLEAR: [&str; 6] = ["clear that", "clear it", "clear the marks", "clear the screen", "clear", "clear those"];
    if NEXT.contains(&t) {
        Some(Said::Next)
    } else if CLEAR.contains(&t) {
        Some(Said::Clear)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_and_their_marks_come_apart() {
        let reply = "Here's how.\nStep 1: Click File [[arrow 300 200 -80 -60 red File]] at the top.\nStep 2: Then Save as [[box 40 120 200 30 green]].\nStep 3: Pick a folder.";
        let steps = parse(reply).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].say, "Here's how. Step 1: Click File at the top.");
        assert_eq!(steps[0].marks[0]["shape"], "arrow");
        assert_eq!(steps[0].marks[0]["w"], -80.0);
        assert_eq!(steps[0].marks[0]["label"], "File");
        assert_eq!(steps[1].marks[0]["color"], "green");
        assert!(steps[2].marks.is_empty());
        assert_eq!(words(&steps[0], 0, 3), "Here's how. Step 1: Click File at the top. (Say \"next\" for step 2 of 3.)");
        assert_eq!(words(&steps[2], 2, 3), "Step 3: Pick a folder.");
    }

    #[test]
    fn one_answer_with_marks_and_plain_answers() {
        let s = parse("It's the red button [[circle 10 10 40 40]] on the left.").unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].say, "It's the red button on the left.");
        assert_eq!(s[0].marks[0]["color"], "red");
        assert!(parse("Nothing to point at here.").is_none());
        assert!(parse("[[sparkle 1 2 3 4]] bad shapes are dropped").is_none());
        let w = parse("[[write 500 40 0 24 blue Saved!]]").unwrap();
        assert_eq!(w[0].marks[0]["text"], "Saved!");
    }

    #[test]
    fn only_step_one_is_spoken_and_never_a_tag() {
        let reply = "Step 1: Click File [[arrow 1 2 3 4]].\nStep 2: Save.";
        assert_eq!(speakable(reply), "Step 1: Click File.");
        // Streaming: an unfinished tag is not read out.
        assert_eq!(speakable("Click here [[arrow 1 2"), "Click here");
        assert_eq!(speakable("No steps at all."), "No steps at all.");
        // "step" inside a sentence isn't a step.
        assert_eq!(speakable("Take one step 2 the left."), "Take one step 2 the left.");
    }

    #[test]
    fn next_and_clear_are_heard() {
        for t in ["Next", "next step.", "Hey Groovy, next", "OK, go on"] {
            assert_eq!(command(t), Some(Said::Next), "{t}");
        }
        for t in ["Clear that", "clear the marks!"] {
            assert_eq!(command(t), Some(Said::Clear), "{t}");
        }
        for t in ["next week remind me", "what's on my screen", "clear my calendar"] {
            assert_eq!(command(t), None, "{t}");
        }
    }
}
