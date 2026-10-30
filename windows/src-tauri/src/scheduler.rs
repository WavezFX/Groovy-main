// One scheduler instead of always-on polling loops (round 3F item 4).
//
// One thread, "groovy-scheduler", runs every periodic job. A job says how long
// until it should run next, from what is true now (`Next`): every so often,
// at an exact moment (a reminder), or not at all until something changes
// (its feature off, nothing to watch). What changes it (an account connected,
// a reminder added, the island opened, a model started) pokes the job, so
// nothing has to poll for it. While the island is folded and nothing is
// playing, no polling job runs more often than every `QUIET_FLOOR`.
//
// Jobs are quick: one that does network or disk work starts it on the async
// runtime and returns (`spawn_once` keeps two of the same from overlapping).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

/// When a job wants to run next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Next {
    /// A poll: again after this long (at least `QUIET_FLOOR` while quiet).
    Every(Duration),
    /// A deadline: after exactly this long, quiet or not.
    In(Duration),
    /// Not until it is poked.
    Idle,
}

/// While the island is folded and nothing plays, polls wait at least this long.
pub const QUIET_FLOOR: Duration = Duration::from_secs(30);

/// A job that is idle is still looked at this often, in case a change came
/// without a poke.
const IDLE_RECHECK: Duration = Duration::from_secs(300);

struct Job {
    name: &'static str,
    next: fn(&AppHandle) -> Next,
    run: fn(&AppHandle),
    due: Instant,
    /// Poked since it last ran: it runs once even if its `next` is Idle (a
    /// feature just turned off still gets to say so).
    poked: bool,
}

struct Jobs {
    list: Mutex<Vec<Job>>,
    changed: Condvar,
}

fn jobs() -> &'static Jobs {
    static J: OnceLock<Jobs> = OnceLock::new();
    J.get_or_init(|| Jobs { list: Mutex::new(Vec::new()), changed: Condvar::new() })
}

/// The island is folded and no music plays: polls slow to `QUIET_FLOOR`.
pub fn quiet(app: &AppHandle) -> bool {
    let folded = app.try_state::<crate::Shared>().is_none_or(|s| s.gate.collapsed.load(Ordering::Relaxed));
    folded && !crate::music::playing() && crate::spotify::playing() != Some(true)
}

/// Whether it was quiet when last looked at.
static WAS_QUIET: AtomicBool = AtomicBool::new(true);

/// How long until a job's `next` is due, given whether it's quiet.
pub fn delay(next: Next, quiet: bool) -> Duration {
    match next {
        Next::Every(d) if quiet => d.max(QUIET_FLOOR),
        Next::Every(d) | Next::In(d) => d,
        Next::Idle => IDLE_RECHECK,
    }
}

/// Adds a job, first run after `first`.
pub fn add(name: &'static str, first: Duration, next: fn(&AppHandle) -> Next, run: fn(&AppHandle)) {
    let j = jobs();
    j.list.lock().unwrap().push(Job { name, next, run, due: Instant::now() + first, poked: false });
    j.changed.notify_all();
}

/// Something `name` depends on changed: it works out when to run again now.
pub fn poke(name: &str) {
    let j = jobs();
    let mut list = j.list.lock().unwrap();
    if let Some(job) = list.iter_mut().find(|job| job.name == name) {
        job.due = Instant::now();
        job.poked = true;
    }
    j.changed.notify_all();
}

/// The island opened or folded, or music started or stopped: if that made it
/// quiet or not, every poll's pace changes, so each is looked at again.
pub fn recheck_pace(app: &AppHandle) {
    let q = quiet(app);
    if WAS_QUIET.swap(q, Ordering::Relaxed) == q {
        return;
    }
    let j = jobs();
    let now = Instant::now();
    for job in j.list.lock().unwrap().iter_mut() {
        job.due = job.due.min(now + Duration::from_secs(1));
    }
    j.changed.notify_all();
}

/// Runs `work` on the async runtime unless the last one started by `flag` is
/// still going.
pub fn spawn_once<F>(flag: &'static AtomicBool, work: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    if flag.swap(true, Ordering::AcqRel) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        work.await;
        flag.store(false, Ordering::Release);
    });
}

/// A due job runs `run` if its `next` isn't Idle, or if it was poked; then it
/// is scheduled again from `next`.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("groovy-scheduler".into()).spawn(move || {
        let j = jobs();
        let mut list = j.list.lock().unwrap();
        loop {
            let now = Instant::now();
            let due: Vec<usize> = (0..list.len()).filter(|i| list[*i].due <= now).collect();
            if due.is_empty() {
                let wait = list.iter().map(|job| job.due.saturating_duration_since(now)).min().unwrap_or(IDLE_RECHECK);
                list = j.changed.wait_timeout(list, wait).unwrap().0;
                continue;
            }
            let picked: Vec<(usize, fn(&AppHandle) -> Next, fn(&AppHandle), bool)> = due
                .iter()
                .map(|i| {
                    let job = &mut list[*i];
                    (*i, job.next, job.run, std::mem::take(&mut job.poked))
                })
                .collect();
            drop(list);
            let quiet = quiet(&app);
            let mut after = Vec::new();
            for (i, next, run, poked) in picked {
                if poked || next(&app) != Next::Idle {
                    run(&app);
                }
                after.push((i, Instant::now() + delay(next(&app), quiet)));
            }
            list = j.list.lock().unwrap();
            for (i, at) in after {
                // A poke while it ran wins.
                if list[i].due <= now {
                    list[i].due = at;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polls_slow_down_while_quiet_and_deadlines_dont() {
        let s = Duration::from_secs;
        assert_eq!(delay(Next::Every(s(3)), false), s(3));
        assert_eq!(delay(Next::Every(s(3)), true), QUIET_FLOOR, "folded and silent: never more often than every 30 s");
        assert_eq!(delay(Next::Every(s(180)), true), s(180));
        assert_eq!(delay(Next::In(s(2)), true), s(2), "a reminder fires on time");
        assert_eq!(delay(Next::Idle, false), IDLE_RECHECK);
    }

    #[test]
    fn a_poke_makes_the_job_due_now() {
        fn idle(_: &AppHandle) -> Next {
            Next::Idle
        }
        fn nothing(_: &AppHandle) {}
        add("test-poke", Duration::from_secs(3600), idle, nothing);
        poke("test-poke");
        let list = jobs().list.lock().unwrap();
        let job = list.iter().find(|j| j.name == "test-poke").unwrap();
        assert!(job.due <= Instant::now());
    }

    #[test]
    fn spawn_once_skips_a_second_start_while_the_first_runs() {
        static FLAG: AtomicBool = AtomicBool::new(false);
        let (tx, rx) = std::sync::mpsc::channel::<()>();
        let rx = std::sync::Arc::new(Mutex::new(rx));
        let hold = rx.clone();
        spawn_once(&FLAG, async move {
            let _ = tauri::async_runtime::spawn_blocking(move || hold.lock().unwrap().recv()).await;
        });
        assert!(FLAG.load(Ordering::Acquire));
        static RAN: AtomicBool = AtomicBool::new(false);
        spawn_once(&FLAG, async { RAN.store(true, Ordering::Release) });
        tx.send(()).unwrap();
        for _ in 0..100 {
            if !FLAG.load(Ordering::Acquire) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!FLAG.load(Ordering::Acquire), "free again once the first is done");
        assert!(!RAN.load(Ordering::Acquire), "the second never started");
    }
}
