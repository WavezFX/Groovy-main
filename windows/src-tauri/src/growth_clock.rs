//! Groovy's growth clock (round 3H item 3). It used to be a timer in the
//! island's page, which a hidden or throttled webview could slow or stop; it
//! is now this thread, which counts while Groovy runs and the PC is awake,
//! whatever the windows do (Standby, the island hidden, a full-screen app in
//! front).
//!
//! Once a minute it adds the time since the last tick to the record of the
//! look he wears (`settings.growths[look]`): `activeSeconds`, today's time
//! (`today: { day, seconds }`) and the days that look was worn for at least
//! 30 minutes (`daysWorn`). A long gap (the PC asleep) counts as two ticks at
//! most. Which of the two counts grows him is the page's rule
//! (src/mochi/growth.ts, GROWTH_RULE and settings.growthRule); the stages are
//! worked out there, from these numbers. The records are saved every five
//! minutes, and every tick is sent to the windows as growth-changed.

use serde_json::{json, Map, Value};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

pub const TICK: Duration = Duration::from_secs(60);
/// A long gap (the PC asleep) counts this much at most.
const MAX_STEP: f64 = 2.0 * 60.0;
const SAVE_EVERY: f64 = 5.0 * 60.0;
/// The "days" rule: a day counts once the look was worn this long that day.
pub const DAY_COUNTS_AFTER: f64 = 30.0 * 60.0;
/// The page's GROWTH_CORRECTION: a record started here needs no correction.
const CORRECTION: u64 = 2;

/// The look whose clock runs: Grove, Blaze and Tide were Night, Ember and Frost grown.
pub fn look_of(groovy_look: &str) -> &'static str {
    match groovy_look {
        "ember" | "blaze" => "ember",
        "frost" | "tide" => "frost",
        _ => "night",
    }
}

/// Today's date where the user is, "2026-10-30".
pub fn local_day() -> String {
    let t = crate::platform::local_time();
    format!("{:04}-{:02}-{:02}", t.year, t.month, t.day)
}

/// Adds `seconds` of running (capped at MAX_STEP) to `look`'s record on `day`
/// (unix time `now`), starting the record if the look hadn't started.
/// Returns the seconds counted.
pub fn tick(growths: &mut Value, look: &str, seconds: f64, day: &str, now: u64) -> f64 {
    let add = if seconds.is_finite() { seconds.clamp(0.0, MAX_STEP) } else { 0.0 };
    if !growths.is_object() {
        *growths = Value::Object(Map::new());
    }
    let all = growths.as_object_mut().expect("an object");
    let record = all.entry(look).or_insert_with(|| {
        json!({ "activeSeconds": 0, "reached": 0, "shown": null, "seen": 0, "hatchedAt": now, "correction": CORRECTION })
    });
    if !record.is_object() {
        *record = json!({ "activeSeconds": 0, "reached": 0, "shown": null, "seen": 0, "hatchedAt": now, "correction": CORRECTION });
    }
    let active = record.get("activeSeconds").and_then(Value::as_f64).filter(|s| s.is_finite() && *s >= 0.0).unwrap_or(0.0);
    record["activeSeconds"] = json!(active + add);
    // Today's time: a new day starts from 0.
    let before = match record.get("today") {
        Some(t) if t.get("day").and_then(Value::as_str) == Some(day) => t.get("seconds").and_then(Value::as_f64).filter(|s| s.is_finite() && *s >= 0.0).unwrap_or(0.0),
        _ => 0.0,
    };
    let after = before + add;
    record["today"] = json!({ "day": day, "seconds": after });
    if before < DAY_COUNTS_AFTER && after >= DAY_COUNTS_AFTER {
        let days = record.get("daysWorn").and_then(Value::as_u64).unwrap_or(0);
        record["daysWorn"] = json!(days + 1);
    }
    add
}

/// Starts the clock. It counts only once he has hatched (the hatchling's
/// look is chosen then, and only its clock starts, round 3E item 12).
pub fn start(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("groovy-growth-clock".into()).spawn(move || {
        let mut last = Instant::now();
        let mut unsaved = 0.0;
        loop {
            std::thread::sleep(TICK);
            let elapsed = last.elapsed().as_secs_f64();
            last = Instant::now();
            let Some(shared) = app.try_state::<crate::Shared>() else { continue };
            let (settings, look, counted) = {
                let mut s = shared.settings.lock().unwrap();
                if !s.hatch_done {
                    continue;
                }
                let look = look_of(&s.groovy_look);
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                let counted = tick(&mut s.growths, look, elapsed, &local_day(), now);
                (s.clone(), look, counted)
            };
            #[cfg(feature = "worktest")]
            {
                let r = &settings.growths[look];
                crate::log::line(format!(
                    "growth tick: {look} +{counted:.0} s ({:.0} s in all, {:.0} s today)",
                    r["activeSeconds"].as_f64().unwrap_or(0.0),
                    r["today"]["seconds"].as_f64().unwrap_or(0.0)
                ));
            }
            #[cfg(not(feature = "worktest"))]
            let _ = (look, counted);
            unsaved += elapsed;
            if unsaved >= SAVE_EVERY {
                unsaved = 0.0;
                if let Err(err) = crate::settings::save(&settings) {
                    crate::log::line(format!("could not save settings: {err}"));
                }
            }
            let _ = app.emit("growth-changed", settings.growths);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    /// `minutes` of ticks on `day`.
    fn run(g: &mut Value, look: &str, day: &str, minutes: u32) {
        for _ in 0..minutes {
            tick(g, look, 60.0, day, NOW);
        }
    }

    #[test]
    fn a_look_worn_two_hours_on_three_evenings_counts_six_hours_and_the_others_stay_not_started() {
        let mut g = json!({});
        for day in ["2026-10-27", "2026-10-28", "2026-10-29"] {
            run(&mut g, "night", day, 120);
        }
        assert_eq!(g["night"]["activeSeconds"], json!(6.0 * 3600.0));
        assert_eq!(g["night"]["daysWorn"], json!(3), "each evening was over 30 minutes");
        assert_eq!(g["night"]["today"], json!({ "day": "2026-10-29", "seconds": 7200.0 }));
        assert!(g.get("ember").is_none() && g.get("frost").is_none(), "only the worn look grows: {g}");
    }

    #[test]
    fn a_first_tick_starts_the_look_like_the_page_does() {
        let mut g = json!({ "night": { "activeSeconds": 100, "reached": 0, "shown": null, "seen": 0, "hatchedAt": 5, "correction": 2 } });
        tick(&mut g, "frost", 60.0, "2026-10-30", NOW);
        assert_eq!(g["frost"]["hatchedAt"], json!(NOW));
        assert_eq!(g["frost"]["correction"], json!(2));
        assert_eq!(g["frost"]["activeSeconds"], json!(60.0));
        assert_eq!(g["night"]["activeSeconds"], json!(100), "the other look is untouched");
    }

    #[test]
    fn a_long_gap_counts_two_ticks_at_most_and_nothing_odd_counts() {
        let mut g = json!({});
        assert_eq!(tick(&mut g, "night", 8.0 * 3600.0, "2026-10-30", NOW), 120.0, "the PC was asleep");
        assert_eq!(tick(&mut g, "night", f64::NAN, "2026-10-30", NOW), 0.0);
        assert_eq!(tick(&mut g, "night", -5.0, "2026-10-30", NOW), 0.0);
        assert_eq!(g["night"]["activeSeconds"], json!(120.0));
    }

    #[test]
    fn a_day_counts_once_after_thirty_minutes_and_today_starts_again_each_day() {
        let mut g = json!({});
        run(&mut g, "ember", "2026-10-30", 29);
        assert!(g["ember"].get("daysWorn").is_none(), "29 minutes isn't a day");
        run(&mut g, "ember", "2026-10-30", 200);
        assert_eq!(g["ember"]["daysWorn"], json!(1), "counted once, however long");
        run(&mut g, "ember", "2026-10-31", 10);
        assert_eq!(g["ember"]["today"], json!({ "day": "2026-10-31", "seconds": 600.0 }));
        assert_eq!(g["ember"]["daysWorn"], json!(1));
    }

    #[test]
    fn the_worn_look_is_one_of_the_three() {
        assert_eq!(look_of("tide"), "frost");
        assert_eq!(look_of("blaze"), "ember");
        assert_eq!(look_of("night"), "night");
        assert_eq!(look_of(""), "night");
    }

    #[test]
    fn a_broken_record_is_started_again() {
        let mut g = json!("nonsense");
        tick(&mut g, "night", 60.0, "2026-10-30", NOW);
        assert_eq!(g["night"]["activeSeconds"], json!(60.0));
        let mut g = json!({ "night": 3 });
        tick(&mut g, "night", 60.0, "2026-10-30", NOW);
        assert_eq!(g["night"]["activeSeconds"], json!(60.0));
    }

    #[test]
    fn local_day_is_a_date() {
        let d = local_day();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
    }
}
