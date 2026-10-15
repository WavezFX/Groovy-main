// Groovy's own music player: the user's music folders, indexed in the local
// database (db.rs), played through the OS audio output with rodio. Works with
// no network at all. MP3, FLAC, Ogg Vorbis, WAV and AAC/M4A.
//
//   library    folders the user added (picked in a dialog, never typed paths
//              from the page), scanned for audio files; title, artist, album,
//              genre, duration and track number read from the tags (lofty),
//              the file name when there are none. A rescan only re-reads files
//              whose modification time changed, and forgets deleted ones.
//   playlists  named, ordered; favourites are a flag on the track.
//   player     one thread owns the audio output. It plays a queue of track
//              ids with shuffle, repeat (off / all / one), seek and volume,
//              goes to the next track by itself, and saves where it was so
//              "resume" picks up after a restart.
//   voice      "play my favourite songs", "pause the music", "next song",
//              "play the playlist Road trip", "lower the volume", "play some
//              relaxing music" — parse_command() below; the chat answers with
//              what really happened, never a pretend success.
//
// Only files under a library folder are ever opened: the page names tracks by
// id, and a track's path comes from the scan.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::{db, log};

pub const EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "oga", "wav", "m4a", "aac"];
const MAX_FILES: usize = 50_000;
const MAX_DEPTH: usize = 12;
const STATE_EVENT: &str = "music-state";
const SCAN_EVENT: &str = "music-scan";

fn sql(e: rusqlite::Error) -> String {
    e.to_string()
}

// ── Library ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub duration: f64,
    pub track_no: Option<i64>,
    pub has_art: bool,
    pub favourite: bool,
    pub play_count: i64,
}

const TRACK_COLS: &str = "id, title, artist, album, genre, duration, track_no, has_art, favourite, play_count";

fn row_track(r: &rusqlite::Row) -> rusqlite::Result<Track> {
    Ok(Track {
        id: r.get(0)?,
        title: r.get(1)?,
        artist: r.get(2)?,
        album: r.get(3)?,
        genre: r.get(4)?,
        duration: r.get(5)?,
        track_no: r.get(6)?,
        has_art: r.get::<_, i64>(7)? != 0,
        favourite: r.get::<_, i64>(8)? != 0,
        play_count: r.get(9)?,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: i64,
    pub path: String,
    pub tracks: i64,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub tracks: i64,
    pub created_at: i64,
}

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Every audio file under `dir`, symlinks not followed, at most MAX_FILES.
pub fn audio_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            if out.len() >= MAX_FILES {
                return out;
            }
            let Ok(meta) = std::fs::symlink_metadata(e.path()) else { continue };
            if meta.is_dir() && depth < MAX_DEPTH {
                stack.push((e.path(), depth + 1));
            } else if meta.is_file() && is_audio(&e.path()) {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

#[derive(Debug, Default, PartialEq)]
pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub duration: f64,
    pub track_no: Option<i64>,
    pub has_art: bool,
}

/// The tags of one file, the file name standing in for a missing title.
pub fn read_tags(path: &Path) -> Tags {
    use lofty::prelude::*;
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut tags = Tags { title: stem, ..Default::default() };
    let Ok(file) = lofty::read_from_path(path) else { return tags };
    tags.duration = file.properties().duration().as_secs_f64();
    if let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) {
        let text = |v: Option<std::borrow::Cow<str>>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        if let Some(t) = text(tag.title()) {
            tags.title = t;
        }
        tags.artist = text(tag.artist()).unwrap_or_default();
        tags.album = text(tag.album()).unwrap_or_default();
        tags.genre = text(tag.genre()).unwrap_or_default();
        tags.track_no = tag.track().map(i64::from);
        tags.has_art = !tag.pictures().is_empty();
    }
    tags
}

fn mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn add_folder(conn: &Connection, path: &Path) -> Result<i64, String> {
    let path = path.canonicalize().map_err(|e| format!("{}: {e}", path.display()))?;
    if !path.is_dir() {
        return Err("Not a folder.".into());
    }
    let p = path.to_string_lossy().to_string();
    conn.execute("INSERT OR IGNORE INTO music_folders (path) VALUES (?1)", [&p]).map_err(sql)?;
    conn.query_row("SELECT id FROM music_folders WHERE path = ?1", [&p], |r| r.get(0)).map_err(sql)
}

pub fn folders(conn: &Connection) -> Result<Vec<Folder>, String> {
    let mut stmt = conn
        .prepare("SELECT f.id, f.path, (SELECT count(*) FROM tracks t WHERE t.folder_id = f.id) FROM music_folders f ORDER BY f.path")
        .map_err(sql)?;
    let rows = stmt
        .query_map([], |r| {
            let path: String = r.get(1)?;
            Ok(Folder { id: r.get(0)?, exists: Path::new(&path).is_dir(), path, tracks: r.get(2)? })
        })
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

pub fn remove_folder(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM music_folders WHERE id = ?1", [id]).map_err(sql)?;
    Ok(())
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize)]
pub struct ScanReport {
    pub found: usize,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
}

/// Indexes one folder: new and changed files are (re)read, vanished ones dropped.
pub fn scan_folder(conn: &Connection, folder_id: i64, dir: &Path, mut progress: impl FnMut(usize, usize)) -> Result<ScanReport, String> {
    let files = audio_files(dir);
    let mut report = ScanReport { found: files.len(), ..Default::default() };
    let known: std::collections::HashMap<String, i64> = {
        let mut stmt = conn.prepare("SELECT path, mtime FROM tracks WHERE folder_id = ?1").map_err(sql)?;
        let rows = stmt.query_map([folder_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)?
    };
    let now = db::now();
    let tx = conn.unchecked_transaction().map_err(sql)?;
    let mut seen = std::collections::HashSet::new();
    for (i, file) in files.iter().enumerate() {
        let p = file.to_string_lossy().to_string();
        let m = mtime(file);
        seen.insert(p.clone());
        if known.get(&p) == Some(&m) {
            continue;
        }
        let t = read_tags(file);
        let existed = known.contains_key(&p);
        tx.execute(
            "INSERT INTO tracks (folder_id, path, title, artist, album, genre, duration, track_no, has_art, mtime, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(path) DO UPDATE SET title = ?3, artist = ?4, album = ?5, genre = ?6, duration = ?7,
               track_no = ?8, has_art = ?9, mtime = ?10",
            params![folder_id, p, t.title, t.artist, t.album, t.genre, t.duration, t.track_no, t.has_art as i64, m, now],
        )
        .map_err(sql)?;
        if existed {
            report.updated += 1;
        } else {
            report.added += 1;
        }
        if i % 50 == 0 {
            progress(i, files.len());
        }
    }
    for p in known.keys().filter(|p| !seen.contains(*p)) {
        tx.execute("DELETE FROM tracks WHERE path = ?1", [p]).map_err(sql)?;
        report.removed += 1;
    }
    tx.commit().map_err(sql)?;
    progress(files.len(), files.len());
    Ok(report)
}

/// Tracks matching `query` (title, artist, album or genre), favourites only if asked.
pub fn search(conn: &Connection, query: &str, favourites: bool, limit: usize) -> Result<Vec<Track>, String> {
    let like = format!("%{}%", query.trim().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {TRACK_COLS} FROM tracks
             WHERE (?1 = '%%' OR title LIKE ?1 ESCAPE '\\' OR artist LIKE ?1 ESCAPE '\\' OR album LIKE ?1 ESCAPE '\\' OR genre LIKE ?1 ESCAPE '\\')
               AND (?2 = 0 OR favourite = 1)
             ORDER BY artist COLLATE NOCASE, album COLLATE NOCASE, track_no, title COLLATE NOCASE
             LIMIT ?3"
        ))
        .map_err(sql)?;
    let rows = stmt.query_map(params![like, favourites as i64, limit as i64], row_track).map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

pub fn track(conn: &Connection, id: i64) -> Result<Option<(Track, String)>, String> {
    conn.query_row(&format!("SELECT {TRACK_COLS}, path FROM tracks WHERE id = ?1"), [id], |r| Ok((row_track(r)?, r.get::<_, String>(10)?)))
        .optional()
        .map_err(sql)
}

pub fn set_favourite(conn: &Connection, id: i64, on: bool) -> Result<(), String> {
    conn.execute("UPDATE tracks SET favourite = ?2 WHERE id = ?1", params![id, on as i64]).map_err(sql)?;
    Ok(())
}

fn played(conn: &Connection, id: i64) {
    let _ = conn.execute("UPDATE tracks SET play_count = play_count + 1, last_played = ?2 WHERE id = ?1", params![id, db::now()]);
}

pub fn playlists(conn: &Connection) -> Result<Vec<Playlist>, String> {
    let mut stmt = conn
        .prepare("SELECT p.id, p.name, (SELECT count(*) FROM playlist_tracks x WHERE x.playlist_id = p.id), p.created_at FROM playlists p ORDER BY p.name COLLATE NOCASE")
        .map_err(sql)?;
    let rows = stmt
        .query_map([], |r| Ok(Playlist { id: r.get(0)?, name: r.get(1)?, tracks: r.get(2)?, created_at: r.get(3)? }))
        .map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

pub fn create_playlist(conn: &Connection, name: &str) -> Result<i64, String> {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(80).collect();
    if name.is_empty() {
        return Err("A playlist needs a name.".into());
    }
    conn.execute("INSERT INTO playlists (name, created_at) VALUES (?1, ?2)", params![name, db::now()]).map_err(sql)?;
    Ok(conn.last_insert_rowid())
}

pub fn rename_playlist(conn: &Connection, id: i64, name: &str) -> Result<(), String> {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(80).collect();
    if name.is_empty() {
        return Err("A playlist needs a name.".into());
    }
    conn.execute("UPDATE playlists SET name = ?2 WHERE id = ?1", params![id, name]).map_err(sql)?;
    Ok(())
}

pub fn delete_playlist(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM playlists WHERE id = ?1", [id]).map_err(sql)?;
    Ok(())
}

pub fn playlist_tracks(conn: &Connection, id: i64) -> Result<Vec<Track>, String> {
    let cols = TRACK_COLS.split(", ").map(|c| format!("t.{c}")).collect::<Vec<_>>().join(", ");
    let mut stmt = conn
        .prepare(&format!("SELECT {cols} FROM playlist_tracks x JOIN tracks t ON t.id = x.track_id WHERE x.playlist_id = ?1 ORDER BY x.position"))
        .map_err(sql)?;
    let rows = stmt.query_map([id], row_track).map_err(sql)?;
    rows.collect::<Result<_, _>>().map_err(sql)
}

pub fn playlist_add(conn: &Connection, id: i64, track_ids: &[i64]) -> Result<(), String> {
    let next: i64 = conn
        .query_row("SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1", [id], |r| r.get(0))
        .map_err(sql)?;
    for (i, t) in track_ids.iter().enumerate() {
        conn.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, position) SELECT ?1, ?2, ?3 WHERE EXISTS (SELECT 1 FROM tracks WHERE id = ?2)",
            params![id, t, next + i as i64],
        )
        .map_err(sql)?;
    }
    Ok(())
}

/// Removes the entry at `position` and closes the gap.
pub fn playlist_remove(conn: &Connection, id: i64, position: i64) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(sql)?;
    tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND position = ?2", params![id, position]).map_err(sql)?;
    tx.execute("UPDATE playlist_tracks SET position = position - 1 WHERE playlist_id = ?1 AND position > ?2", params![id, position])
        .map_err(sql)?;
    tx.commit().map_err(sql)
}

/// The track ids of a playlist found by name: exact first, then contains,
/// then the most recently created one when asked for "the playlist I made yesterday".
pub fn find_playlist(conn: &Connection, wanted: &PlaylistRef) -> Result<Option<Playlist>, String> {
    let all = playlists(conn)?;
    Ok(match wanted {
        PlaylistRef::Named(name) => {
            let n = name.to_lowercase();
            all.iter()
                .find(|p| p.name.to_lowercase() == n)
                .or_else(|| all.iter().find(|p| p.name.to_lowercase().contains(&n) || n.contains(&p.name.to_lowercase())))
                .cloned()
        }
        PlaylistRef::Latest => all.into_iter().max_by_key(|p| (p.created_at, p.id)),
    })
}

/// Words that mark calm music in a genre or a playlist name.
const RELAXING: &[&str] = &[
    "relax", "chill", "calm", "ambient", "lofi", "lo-fi", "acoustic", "classical", "piano", "jazz", "sleep", "study", "downtempo",
    "new age", "meditation", "instrumental",
];

pub fn relaxing_tracks(conn: &Connection) -> Result<Vec<i64>, String> {
    let mut stmt = conn.prepare("SELECT id, genre, album, title FROM tracks").map_err(sql)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, format!("{} {} {}", r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))))
        .map_err(sql)?;
    let mut out = Vec::new();
    for row in rows {
        let (id, text) = row.map_err(sql)?;
        let t = text.to_lowercase();
        if RELAXING.iter().any(|w| t.contains(w)) {
            out.push(id);
        }
    }
    Ok(out)
}

pub fn kv_get(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0)).optional().ok().flatten()
}

pub fn kv_set(conn: &Connection, key: &str, value: &str) {
    let _ = conn.execute(
        "INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
        [key, value],
    );
}

// ── Queue logic (pure) ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

/// Where the queue goes after `index` finished (or "next" was pressed when
/// `manual`). None = the end. `pick` is a random number for shuffle.
pub fn next_index(len: usize, index: usize, repeat: Repeat, shuffle: bool, manual: bool, pick: u64) -> Option<usize> {
    if len == 0 {
        return None;
    }
    if repeat == Repeat::One && !manual {
        return Some(index);
    }
    if shuffle && len > 1 {
        let r = (pick % (len as u64 - 1)) as usize;
        return Some(if r >= index { r + 1 } else { r });
    }
    if index + 1 < len {
        Some(index + 1)
    } else if repeat != Repeat::Off || (manual && shuffle) {
        Some(0)
    } else {
        None
    }
}

/// "Previous": restart the track if more than 3 s in, else the one before.
pub fn prev_index(index: usize, position: f64, len: usize, repeat: Repeat) -> (usize, bool) {
    if position > 3.0 || len == 0 {
        return (index, true);
    }
    if index > 0 {
        (index - 1, false)
    } else if repeat == Repeat::All {
        (len - 1, false)
    } else {
        (0, true)
    }
}

// ── Commands from words ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistRef {
    Named(String),
    Latest,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MusicCommand {
    Pause,
    Resume,
    Next,
    Previous,
    Stop,
    VolumeUp,
    VolumeDown,
    Volume(u8),
    PlayFavourites,
    PlayRelaxing,
    PlayPlaylist(PlaylistRef),
    PlaySearch(String),
    PlayAll,
}

/// A spoken or typed music request, or None when it isn't one. Deliberately
/// narrow: only clear requests about music are taken, anything else goes to
/// the assistant as usual.
pub fn parse_command(text: &str) -> Option<MusicCommand> {
    let mut t = text.trim().to_lowercase();
    t.retain(|c| c != '?' && c != '!' && c != '.');
    for lead in ["hey groovy,", "hey groovy", "groovy,", "groovy", "please", "can you", "could you"] {
        if let Some(rest) = t.strip_prefix(lead) {
            t = rest.trim().trim_start_matches(',').trim().to_string();
        }
    }
    let t = t.trim_end_matches(" please").trim().to_string();
    let music_word = ["music", "song", "track", "playback", "the tune", "this song"].iter().any(|w| t.contains(w));

    match t.as_str() {
        "pause" | "pause it" | "pause the music" | "pause music" | "pause the song" | "stop the music" | "stop music" => {
            return Some(if t.starts_with("stop") { MusicCommand::Stop } else { MusicCommand::Pause })
        }
        "resume" | "resume the music" | "resume music" | "resume playback" | "unpause" | "continue the music" | "keep playing" => {
            return Some(MusicCommand::Resume)
        }
        "next" | "next song" | "next track" | "skip" | "skip this song" | "skip this" | "skip song" | "skip track" => {
            return Some(MusicCommand::Next)
        }
        "previous" | "previous song" | "previous track" | "go back a song" | "last song" | "play the previous song" => {
            return Some(MusicCommand::Previous)
        }
        "turn it up" | "louder" | "volume up" | "raise the volume" | "increase the volume" | "turn up the music" | "turn the music up" => {
            return Some(MusicCommand::VolumeUp)
        }
        "turn it down" | "quieter" | "volume down" | "lower the volume" | "decrease the volume" | "turn down the music" | "turn the music down" => {
            return Some(MusicCommand::VolumeDown)
        }
        "play music" | "play some music" | "play my music" | "play something" => return Some(MusicCommand::PlayAll),
        _ => {}
    }
    if let Some(n) = t.strip_prefix("set the volume to ").or_else(|| t.strip_prefix("volume ")) {
        if let Ok(v) = n.trim_end_matches('%').trim_end_matches(" percent").trim().parse::<u8>() {
            return Some(MusicCommand::Volume(v.min(100)));
        }
    }
    let rest = t.strip_prefix("play ")?;
    let rest = rest.strip_prefix("me ").unwrap_or(rest);
    if ["my favourite", "my favorite", "my favourites", "my favorites", "favourite songs", "favorite songs", "my liked"]
        .iter()
        .any(|w| rest.starts_with(w))
    {
        return Some(MusicCommand::PlayFavourites);
    }
    if RELAXING.iter().any(|w| rest.contains(w)) && (music_word || rest.contains("something") || rest.starts_with("some")) {
        return Some(MusicCommand::PlayRelaxing);
    }
    if rest.contains("playlist") {
        if ["i made yesterday", "i just made", "i made last", "my latest", "my newest", "my last playlist", "the latest", "the newest"]
            .iter()
            .any(|w| rest.contains(w))
        {
            return Some(MusicCommand::PlayPlaylist(PlaylistRef::Latest));
        }
        let name = rest
            .trim_start_matches("the ")
            .trim_start_matches("my ")
            .trim_start_matches("playlist ")
            .trim_end_matches(" playlist")
            .trim_start_matches("called ")
            .trim_start_matches("named ")
            .trim()
            .trim_matches(|c| c == '"' || c == '“' || c == '”' || c == '\'')
            .to_string();
        if !name.is_empty() && name != "playlist" {
            return Some(MusicCommand::PlayPlaylist(PlaylistRef::Named(name)));
        }
        return None;
    }
    // "play Bohemian Rhapsody", "play some Daft Punk", "play the album Discovery"
    let q = rest
        .trim_start_matches("some ")
        .trim_start_matches("the song ")
        .trim_start_matches("the album ")
        .trim_start_matches("songs by ")
        .trim_start_matches("music by ")
        .trim_end_matches(" songs")
        .trim_end_matches(" music")
        .trim();
    // "play a game", "play with me" are not music.
    let not_music = ["game", "video", "movie", "film", "chess", "with me"];
    if q.is_empty() || q.len() > 80 || q.starts_with("with ") || not_music.iter().any(|w| q.contains(w)) {
        return None;
    }
    Some(MusicCommand::PlaySearch(q.to_string()))
}

// ── The player ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub queue: Vec<i64>,
    pub index: Option<usize>,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub track: Option<Track>,
    pub error: Option<String>,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            queue: Vec::new(),
            index: None,
            playing: false,
            position: 0.0,
            duration: 0.0,
            volume: 0.7,
            shuffle: false,
            repeat: Repeat::Off,
            track: None,
            error: None,
        }
    }
}

enum Cmd {
    Load { queue: Vec<i64>, index: usize, at: f64, play: bool },
    Toggle,
    Pause,
    Resume,
    Next,
    Previous,
    Seek(f64),
    Volume(f32),
    Shuffle(bool),
    Repeat(Repeat),
    Stop,
}

struct Player {
    tx: Sender<Cmd>,
    state: Mutex<PlayerState>,
}

static PLAYER: OnceLock<Player> = OnceLock::new();

fn player() -> &'static Player {
    PLAYER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        let p = Player { tx, state: Mutex::new(restored()) };
        std::thread::Builder::new()
            .name("music".into())
            .spawn(move || run(rx))
            .expect("music thread");
        p
    })
}

static APP: OnceLock<AppHandle> = OnceLock::new();

pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
    player();
}

/// Quitting: remember where the music was.
pub fn stop_on_exit() {
    if PLAYER.get().is_some() {
        let mut s = state();
        s.playing = false;
        save(&s);
    }
}

pub fn state() -> PlayerState {
    player().state.lock().unwrap().clone()
}

fn publish() {
    let s = state();
    if let Some(app) = APP.get() {
        let _ = app.emit(STATE_EVENT, &s);
    }
}

fn send(cmd: Cmd) {
    let _ = player().tx.send(cmd);
}

#[derive(Serialize, Deserialize)]
struct Saved {
    queue: Vec<i64>,
    index: usize,
    position: f64,
    volume: f32,
    shuffle: bool,
    repeat: Repeat,
}

fn restored() -> PlayerState {
    let mut s = PlayerState::default();
    let Ok(conn) = db::conn() else { return s };
    let Some(saved) = kv_get(&conn, "music.resume").and_then(|v| serde_json::from_str::<Saved>(&v).ok()) else { return s };
    s.volume = saved.volume.clamp(0.0, 1.0);
    s.shuffle = saved.shuffle;
    s.repeat = saved.repeat;
    if saved.index < saved.queue.len() {
        s.track = track(&conn, saved.queue[saved.index]).ok().flatten().map(|(t, _)| t);
        s.duration = s.track.as_ref().map(|t| t.duration).unwrap_or(0.0);
        s.index = Some(saved.index);
        s.position = saved.position.max(0.0);
        s.queue = saved.queue;
    }
    s
}

fn save(s: &PlayerState) {
    let Ok(conn) = db::conn() else { return };
    let saved = Saved {
        queue: s.queue.iter().copied().take(5000).collect(),
        index: s.index.unwrap_or(0),
        position: s.position,
        volume: s.volume,
        shuffle: s.shuffle,
        repeat: s.repeat,
    };
    if let Ok(v) = serde_json::to_string(&saved) {
        kv_set(&conn, "music.resume", &v);
    }
}

fn random() -> u64 {
    // xorshift seeded from the clock: good enough to shuffle songs.
    let mut x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7) | 1;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    x
}

/// The audio thread. The output device is opened on the first play, so a
/// machine with no sound card only fails when asked to play.
fn run(rx: Receiver<Cmd>) {
    let mut output: Option<(rodio::OutputStream, rodio::OutputStreamHandle)> = None;
    let mut sink: Option<rodio::Sink> = None;
    let mut last_save = Instant::now();

    let set = |f: &mut dyn FnMut(&mut PlayerState)| {
        let mut s = player().state.lock().unwrap();
        f(&mut s);
    };

    // Starts track `index` of the queue at `at` seconds. False when it can't be played.
    let start = |index: usize, at: f64, play: bool, output: &mut Option<(rodio::OutputStream, rodio::OutputStreamHandle)>, sink: &mut Option<rodio::Sink>| -> bool {
        let (id, volume) = {
            let s = player().state.lock().unwrap();
            (s.queue.get(index).copied(), s.volume)
        };
        let Some(id) = id else { return false };
        let found = db::conn().ok().and_then(|c| track(&c, id).ok().flatten());
        let Some((t, path)) = found else {
            set(&mut |s| s.error = Some("That track is no longer in the library.".into()));
            return false;
        };
        if output.is_none() {
            match rodio::OutputStream::try_default() {
                Ok(o) => *output = Some(o),
                Err(e) => {
                    set(&mut |s| {
                        s.error = Some(format!("No audio output: {e}"));
                        s.playing = false;
                    });
                    return false;
                }
            }
        }
        let handle = &output.as_ref().unwrap().1;
        let file = match std::fs::File::open(&path) {
            Ok(f) => f,
            Err(e) => {
                set(&mut |s| s.error = Some(format!("{}: {e}", t.title)));
                return false;
            }
        };
        let decoder = match rodio::Decoder::new(std::io::BufReader::new(file)) {
            Ok(d) => d,
            Err(e) => {
                set(&mut |s| s.error = Some(format!("{}: {e}", t.title)));
                return false;
            }
        };
        let new_sink = match rodio::Sink::try_new(handle) {
            Ok(s) => s,
            Err(e) => {
                set(&mut |s| s.error = Some(format!("No audio output: {e}")));
                return false;
            }
        };
        new_sink.set_volume(volume);
        new_sink.append(decoder);
        if at > 0.5 {
            let _ = new_sink.try_seek(Duration::from_secs_f64(at));
        }
        if play {
            new_sink.play();
        } else {
            new_sink.pause();
        }
        if let Some(old) = sink.replace(new_sink) {
            old.stop();
        }
        if let Ok(c) = db::conn() {
            if play && at < 1.0 {
                played(&c, id);
            }
        }
        let duration = t.duration;
        set(&mut |s| {
            s.index = Some(index);
            s.track = Some(t.clone());
            s.duration = duration;
            s.position = at;
            s.playing = play;
            s.error = None;
        });
        true
    };

    loop {
        let cmd = match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(c) => Some(c),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let mut changed = cmd.is_some();
        match cmd {
            Some(Cmd::Load { queue, index, at, play }) => {
                set(&mut |s| s.queue = queue.clone());
                // A file that can't be played is skipped, up to the whole queue.
                let len = queue.len();
                let mut i = index;
                for _ in 0..len.max(1) {
                    if start(i, at, play, &mut output, &mut sink) {
                        break;
                    }
                    i = (i + 1) % len.max(1);
                }
            }
            Some(Cmd::Toggle) | Some(Cmd::Resume) | Some(Cmd::Pause) => {
                let (playing, index, position, has_queue) = {
                    let s = player().state.lock().unwrap();
                    (s.playing, s.index, s.position, !s.queue.is_empty())
                };
                let want = match cmd {
                    Some(Cmd::Pause) => false,
                    Some(Cmd::Resume) => true,
                    _ => !playing,
                };
                match (&sink, want) {
                    (Some(sk), true) => sk.play(),
                    (Some(sk), false) => sk.pause(),
                    // Nothing loaded yet (after a restart): load where it was.
                    (None, true) if has_queue => {
                        start(index.unwrap_or(0), position, true, &mut output, &mut sink);
                    }
                    _ => {}
                }
                if sink.is_some() {
                    set(&mut |s| s.playing = want);
                }
            }
            Some(Cmd::Next) | Some(Cmd::Previous) => {
                let (len, index, position, repeat, shuffle, playing) = {
                    let s = player().state.lock().unwrap();
                    (s.queue.len(), s.index.unwrap_or(0), s.position, s.repeat, s.shuffle, s.playing || sink.is_none())
                };
                let target = if matches!(cmd, Some(Cmd::Next)) {
                    next_index(len, index, repeat, shuffle, true, random())
                } else {
                    Some(prev_index(index, position, len, repeat).0)
                };
                if let Some(i) = target {
                    start(i, 0.0, playing, &mut output, &mut sink);
                }
            }
            Some(Cmd::Seek(at)) => {
                if let Some(sk) = &sink {
                    let _ = sk.try_seek(Duration::from_secs_f64(at.max(0.0)));
                }
                set(&mut |s| s.position = at.max(0.0));
            }
            Some(Cmd::Volume(v)) => {
                let v = v.clamp(0.0, 1.0);
                if let Some(sk) = &sink {
                    sk.set_volume(v);
                }
                set(&mut |s| s.volume = v);
            }
            Some(Cmd::Shuffle(on)) => set(&mut |s| s.shuffle = on),
            Some(Cmd::Repeat(r)) => set(&mut |s| s.repeat = r),
            Some(Cmd::Stop) => {
                if let Some(sk) = sink.take() {
                    sk.stop();
                }
                set(&mut |s| {
                    s.playing = false;
                    s.position = 0.0;
                });
                // Free the audio device while nothing plays.
                output = None;
            }
            None => {}
        }

        // Position, and the end of a track.
        if let Some(sk) = &sink {
            let pos = sk.get_pos().as_secs_f64();
            let finished = sk.empty();
            let (playing, len, index, repeat, shuffle) = {
                let mut s = player().state.lock().unwrap();
                if (s.position - pos).abs() > 0.9 {
                    changed = true;
                }
                s.position = pos;
                (s.playing, s.queue.len(), s.index.unwrap_or(0), s.repeat, s.shuffle)
            };
            if finished && playing {
                match next_index(len, index, repeat, shuffle, false, random()) {
                    Some(i) => {
                        start(i, 0.0, true, &mut output, &mut sink);
                    }
                    None => {
                        sink = None;
                        set(&mut |s| {
                            s.playing = false;
                            s.position = 0.0;
                        });
                    }
                }
                changed = true;
            }
        }
        if changed {
            publish();
        }
        if changed || last_save.elapsed() > Duration::from_secs(10) {
            if last_save.elapsed() > Duration::from_secs(2) || changed {
                save(&state());
                last_save = Instant::now();
            }
        }
    }
}

/// "Play <something>" is only taken when the library has it: "play chess with
/// me" or a song only Spotify has goes to the assistant instead.
pub fn worth_handling(cmd: &MusicCommand) -> bool {
    match cmd {
        MusicCommand::PlaySearch(q) => db::conn().ok().is_some_and(|c| search(&c, q, false, 1).is_ok_and(|r| !r.is_empty())),
        _ => true,
    }
}

/// Runs a command parsed from words and says what really happened.
pub fn execute(cmd: &MusicCommand) -> Result<String, String> {
    let conn = db::conn()?;
    let has_library: i64 = conn.query_row("SELECT count(*) FROM tracks", [], |r| r.get(0)).map_err(sql)?;
    let s = state();
    let none_loaded = s.queue.is_empty();
    let no_music = || crate::i18n::t("There's no music in your library yet: add a folder in Settings → Music.");
    let play_ids = |ids: Vec<i64>, what: &str| -> Result<String, String> {
        if ids.is_empty() {
            return Err(crate::i18n::tf("I couldn't find {what} in your music library.", &[("what", what)]));
        }
        let n = ids.len().to_string();
        send(Cmd::Load { queue: ids, index: 0, at: 0.0, play: true });
        Ok(crate::i18n::tf("Playing {what} ({n} tracks).", &[("what", what), ("n", &n)]))
    };
    match cmd {
        MusicCommand::Pause | MusicCommand::Stop if none_loaded || !s.playing => Ok(crate::i18n::t("Nothing is playing.")),
        MusicCommand::Pause => {
            send(Cmd::Pause);
            Ok(crate::i18n::t("Paused."))
        }
        MusicCommand::Stop => {
            send(Cmd::Stop);
            Ok(crate::i18n::t("Stopped."))
        }
        MusicCommand::Resume if none_loaded => Err(no_music_or(has_library, crate::i18n::n_("Nothing to resume: ask me to play something."))),
        MusicCommand::Resume => {
            send(Cmd::Resume);
            Ok(crate::i18n::t("Playing."))
        }
        MusicCommand::Next | MusicCommand::Previous if none_loaded => Ok(crate::i18n::t("Nothing is playing.")),
        MusicCommand::Next => {
            send(Cmd::Next);
            Ok(crate::i18n::t("Next track."))
        }
        MusicCommand::Previous => {
            send(Cmd::Previous);
            Ok(crate::i18n::t("Previous track."))
        }
        MusicCommand::VolumeUp | MusicCommand::VolumeDown | MusicCommand::Volume(_) => {
            let v = match cmd {
                MusicCommand::VolumeUp => (s.volume + 0.15).min(1.0),
                MusicCommand::VolumeDown => (s.volume - 0.15).max(0.0),
                MusicCommand::Volume(p) => *p as f32 / 100.0,
                _ => s.volume,
            };
            send(Cmd::Volume(v));
            let pct = ((v * 100.0).round() as u32).to_string();
            Ok(crate::i18n::tf("Volume {pct}%.", &[("pct", &pct)]))
        }
        _ if has_library == 0 => Err(no_music()),
        MusicCommand::PlayAll => {
            let mut ids: Vec<i64> = search(&conn, "", false, 5000)?.into_iter().map(|t| t.id).collect();
            shuffle(&mut ids);
            play_ids(ids, &crate::i18n::t("your music"))
        }
        MusicCommand::PlayFavourites => {
            let mut ids: Vec<i64> = search(&conn, "", true, 5000)?.into_iter().map(|t| t.id).collect();
            if ids.is_empty() {
                return Err(crate::i18n::t("You have no favourite tracks yet: star some in Settings → Music."));
            }
            shuffle(&mut ids);
            play_ids(ids, &crate::i18n::t("your favourites"))
        }
        MusicCommand::PlayRelaxing => {
            let mut ids = relaxing_tracks(&conn)?;
            if ids.is_empty() {
                // A playlist named for it counts too.
                if let Some(p) = find_playlist(&conn, &PlaylistRef::Named("relax".into()))?.or(find_playlist(&conn, &PlaylistRef::Named("chill".into()))?) {
                    ids = playlist_tracks(&conn, p.id)?.into_iter().map(|t| t.id).collect();
                }
            }
            if ids.is_empty() {
                return Err(crate::i18n::t("I couldn't find relaxing music: no tracks have a calm genre (ambient, chill, classical, jazz…) and there's no “relax” or “chill” playlist."));
            }
            shuffle(&mut ids);
            play_ids(ids, &crate::i18n::t("something relaxing"))
        }
        MusicCommand::PlayPlaylist(r) => {
            let Some(p) = find_playlist(&conn, r)? else {
                return Err(match r {
                    PlaylistRef::Named(n) => crate::i18n::tf("There's no playlist called “{name}”.", &[("name", n)]),
                    PlaylistRef::Latest => crate::i18n::t("You haven't made a playlist yet."),
                });
            };
            let ids: Vec<i64> = playlist_tracks(&conn, p.id)?.into_iter().map(|t| t.id).collect();
            if ids.is_empty() {
                return Err(crate::i18n::tf("The playlist “{name}” is empty.", &[("name", &p.name)]));
            }
            play_ids(ids, &format!("“{}”", p.name))
        }
        MusicCommand::PlaySearch(q) => {
            let ids: Vec<i64> = search(&conn, q, false, 500)?.into_iter().map(|t| t.id).collect();
            play_ids(ids, &format!("“{q}”"))
        }
    }
}

fn no_music_or(has_library: i64, otherwise: &str) -> String {
    if has_library == 0 {
        crate::i18n::t("There's no music in your library yet: add a folder in Settings → Music.")
    } else {
        crate::i18n::t(otherwise)
    }
}

fn shuffle(ids: &mut [i64]) {
    for i in (1..ids.len()).rev() {
        let j = (random() % (i as u64 + 1)) as usize;
        ids.swap(i, j);
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Opens the OS folder picker and adds what was picked, then scans it.
#[tauri::command]
pub async fn music_add_folder(app: AppHandle) -> Result<Option<ScanReport>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title(crate::i18n::t("Add a music folder")).pick_folder(move |picked| {
        let _ = tx.send(picked);
    });
    let Some(picked) = rx.await.map_err(|e| e.to_string())? else { return Ok(None) };
    let path = picked.into_path().map_err(|e| e.to_string())?;
    let id = add_folder(&*db::conn()?, &path)?;
    log::line("music: folder added");
    let report = scan(app, Some(id)).await?;
    Ok(Some(report))
}

async fn scan(app: AppHandle, only: Option<i64>) -> Result<ScanReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let list = folders(&*db::conn()?)?;
        let mut total = ScanReport::default();
        for f in list.into_iter().filter(|f| only.is_none_or(|id| id == f.id)) {
            if !f.exists {
                continue;
            }
            let conn = db::conn()?;
            let r = scan_folder(&conn, f.id, Path::new(&f.path), |done, of| {
                let _ = app.emit(SCAN_EVENT, serde_json::json!({ "done": done, "total": of }));
            })?;
            total.found += r.found;
            total.added += r.added;
            total.updated += r.updated;
            total.removed += r.removed;
        }
        log::line(format!("music: scanned, {} files ({} new, {} changed, {} gone)", total.found, total.added, total.updated, total.removed));
        Ok(total)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn music_rescan(app: AppHandle) -> Result<ScanReport, String> {
    scan(app, None).await
}

#[tauri::command]
pub fn music_folders() -> Result<Vec<Folder>, String> {
    folders(&*db::conn()?)
}

#[tauri::command]
pub fn music_remove_folder(id: i64) -> Result<(), String> {
    remove_folder(&*db::conn()?, id)
}

#[tauri::command]
pub fn music_search(query: String, favourites: bool) -> Result<Vec<Track>, String> {
    search(&*db::conn()?, &query, favourites, 2000)
}

#[tauri::command]
pub fn music_set_favourite(id: i64, on: bool) -> Result<(), String> {
    set_favourite(&*db::conn()?, id, on)
}

/// A track's embedded cover as a data URL (2 MB at most), or None.
#[tauri::command]
pub fn music_art(id: i64) -> Result<Option<String>, String> {
    use base64_lite::encode;
    use lofty::prelude::*;
    let Some((_, path)) = track(&*db::conn()?, id)? else { return Ok(None) };
    let Ok(file) = lofty::read_from_path(&path) else { return Ok(None) };
    let Some(pic) = file.primary_tag().or_else(|| file.first_tag()).and_then(|t| t.pictures().first().cloned()) else { return Ok(None) };
    let data = pic.data();
    if data.len() > 2 * 1024 * 1024 {
        return Ok(None);
    }
    let mime = match pic.mime_type() {
        Some(lofty::picture::MimeType::Png) => "image/png",
        Some(lofty::picture::MimeType::Gif) => "image/gif",
        Some(lofty::picture::MimeType::Bmp) => "image/bmp",
        _ => "image/jpeg",
    };
    Ok(Some(format!("data:{mime};base64,{}", encode(data))))
}

#[tauri::command]
pub fn music_playlists() -> Result<Vec<Playlist>, String> {
    playlists(&*db::conn()?)
}

#[tauri::command]
pub fn music_playlist_create(name: String) -> Result<i64, String> {
    create_playlist(&*db::conn()?, &name)
}

#[tauri::command]
pub fn music_playlist_rename(id: i64, name: String) -> Result<(), String> {
    rename_playlist(&*db::conn()?, id, &name)
}

#[tauri::command]
pub fn music_playlist_delete(id: i64) -> Result<(), String> {
    delete_playlist(&*db::conn()?, id)
}

#[tauri::command]
pub fn music_playlist_tracks(id: i64) -> Result<Vec<Track>, String> {
    playlist_tracks(&*db::conn()?, id)
}

#[tauri::command]
pub fn music_playlist_add(id: i64, tracks: Vec<i64>) -> Result<(), String> {
    playlist_add(&*db::conn()?, id, &tracks)
}

#[tauri::command]
pub fn music_playlist_remove(id: i64, position: i64) -> Result<(), String> {
    playlist_remove(&*db::conn()?, id, position)
}

#[tauri::command]
pub fn music_state() -> PlayerState {
    state()
}

#[tauri::command]
pub fn music_play(queue: Vec<i64>, index: usize) -> Result<(), String> {
    if queue.is_empty() || index >= queue.len() {
        return Err("Nothing to play.".into());
    }
    send(Cmd::Load { queue: queue.into_iter().take(5000).collect(), index, at: 0.0, play: true });
    Ok(())
}

/// toggle | pause | resume | next | previous | stop
#[tauri::command]
pub fn music_control(action: String) -> Result<(), String> {
    send(match action.as_str() {
        "toggle" => Cmd::Toggle,
        "pause" => Cmd::Pause,
        "resume" => Cmd::Resume,
        "next" => Cmd::Next,
        "previous" => Cmd::Previous,
        "stop" => Cmd::Stop,
        _ => return Err(format!("Unknown action: {action}")),
    });
    Ok(())
}

#[tauri::command]
pub fn music_seek(position: f64) {
    if position.is_finite() {
        send(Cmd::Seek(position));
    }
}

#[tauri::command]
pub fn music_volume(volume: f32) {
    if volume.is_finite() {
        send(Cmd::Volume(volume));
    }
}

#[tauri::command]
pub fn music_set_mode(shuffle: bool, repeat: Repeat) {
    send(Cmd::Shuffle(shuffle));
    send(Cmd::Repeat(repeat));
}

/// Tiny base64 encoder for cover art (no extra crate).
mod base64_lite {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    pub fn encode(data: &[u8]) -> String {
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        for c in data.chunks(3) {
            let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
            out.push(T[(n >> 18) as usize & 63] as char);
            out.push(T[(n >> 12) as usize & 63] as char);
            out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
            out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        db::open_in_memory().unwrap()
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("groovy-music-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// A valid 0.25 s silent 16-bit mono WAV.
    fn wav() -> Vec<u8> {
        let samples = 4000u32;
        let mut b = Vec::new();
        b.extend(b"RIFF");
        b.extend((36 + samples * 2).to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(16000u32.to_le_bytes());
        b.extend(32000u32.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend((samples * 2).to_le_bytes());
        b.extend(vec![0u8; (samples * 2) as usize]);
        b
    }

    #[test]
    fn finds_audio_files_only() {
        let d = tmp("find");
        std::fs::create_dir_all(d.join("Album")).unwrap();
        for f in ["a.mp3", "Album/b.FLAC", "Album/c.ogg", "notes.txt", "cover.jpg", "d.wav", "e.m4a"] {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        let names: Vec<_> = audio_files(&d).iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["b.FLAC", "c.ogg", "a.mp3", "d.wav", "e.m4a"]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn scans_reads_tags_and_forgets_deleted_files() {
        let d = tmp("scan");
        std::fs::write(d.join("Quiet Song.wav"), wav()).unwrap();
        std::fs::write(d.join("broken.mp3"), b"not audio").unwrap();
        let c = conn();
        let id = add_folder(&c, &d).unwrap();
        assert_eq!(add_folder(&c, &d).unwrap(), id, "adding twice is one folder");
        let r = scan_folder(&c, id, &d, |_, _| {}).unwrap();
        assert_eq!((r.found, r.added, r.updated, r.removed), (2, 2, 0, 0));
        let all = search(&c, "", false, 100).unwrap();
        let quiet = all.iter().find(|t| t.title == "Quiet Song").expect("title from the file name");
        assert!((quiet.duration - 0.25).abs() < 0.05, "duration {}", quiet.duration);
        // Unchanged files are not read again.
        let again = scan_folder(&c, id, &d, |_, _| {}).unwrap();
        assert_eq!((again.added, again.updated, again.removed), (0, 0, 0));
        std::fs::remove_file(d.join("broken.mp3")).unwrap();
        assert_eq!(scan_folder(&c, id, &d, |_, _| {}).unwrap().removed, 1);
        assert_eq!(folders(&c).unwrap()[0].tracks, 1);
        remove_folder(&c, id).unwrap();
        assert!(search(&c, "", false, 100).unwrap().is_empty(), "a folder's tracks go with it");
        let _ = std::fs::remove_dir_all(&d);
    }

    fn insert(c: &Connection, title: &str, artist: &str, genre: &str) -> i64 {
        c.execute("INSERT OR IGNORE INTO music_folders (id, path) VALUES (1, '/m')", []).unwrap();
        c.execute(
            "INSERT INTO tracks (folder_id, path, title, artist, genre) VALUES (1, ?1, ?2, ?3, ?4)",
            params![format!("/m/{title}.mp3"), title, artist, genre],
        )
        .unwrap();
        c.last_insert_rowid()
    }

    #[test]
    fn search_favourites_and_relaxing() {
        let c = conn();
        let a = insert(&c, "One More Time", "Daft Punk", "House");
        let b = insert(&c, "Clair de Lune", "Debussy", "Classical");
        insert(&c, "100%_literal", "Test", "Rock");
        assert_eq!(search(&c, "daft", false, 10).unwrap()[0].id, a);
        assert_eq!(search(&c, "classical", false, 10).unwrap()[0].id, b);
        assert_eq!(search(&c, "%", false, 10).unwrap().len(), 1, "wildcards are literal");
        assert_eq!(search(&c, "_", false, 10).unwrap().len(), 1);
        set_favourite(&c, a, true).unwrap();
        assert_eq!(search(&c, "", true, 10).unwrap().iter().map(|t| t.id).collect::<Vec<_>>(), vec![a]);
        assert_eq!(relaxing_tracks(&c).unwrap(), vec![b]);
    }

    #[test]
    fn playlists_keep_their_order() {
        let c = conn();
        let a = insert(&c, "A", "", "");
        let b = insert(&c, "B", "", "");
        let p = create_playlist(&c, "  Road trip ").unwrap();
        assert!(create_playlist(&c, "  ").is_err());
        playlist_add(&c, p, &[a, b, a, 999]).unwrap();
        let titles = |c: &Connection| playlist_tracks(c, p).unwrap().iter().map(|t| t.title.clone()).collect::<Vec<_>>();
        assert_eq!(titles(&c), vec!["A", "B", "A"], "unknown ids are skipped");
        playlist_remove(&c, p, 1).unwrap();
        assert_eq!(titles(&c), vec!["A", "A"]);
        playlist_add(&c, p, &[b]).unwrap();
        assert_eq!(titles(&c), vec!["A", "A", "B"]);
        let found = find_playlist(&c, &PlaylistRef::Named("road trip".into())).unwrap().unwrap();
        assert_eq!(found.name, "Road trip");
        assert_eq!(find_playlist(&c, &PlaylistRef::Named("road".into())).unwrap().unwrap().id, p);
        let newer = create_playlist(&c, "Later").unwrap();
        assert_eq!(find_playlist(&c, &PlaylistRef::Latest).unwrap().unwrap().id, newer);
        rename_playlist(&c, p, "Trip").unwrap();
        delete_playlist(&c, p).unwrap();
        assert_eq!(playlists(&c).unwrap().len(), 1);
    }

    #[test]
    fn queue_moves_with_repeat_and_shuffle() {
        assert_eq!(next_index(3, 0, Repeat::Off, false, false, 0), Some(1));
        assert_eq!(next_index(3, 2, Repeat::Off, false, false, 0), None, "the end");
        assert_eq!(next_index(3, 2, Repeat::All, false, false, 0), Some(0));
        assert_eq!(next_index(3, 1, Repeat::One, false, false, 0), Some(1), "repeat one");
        assert_eq!(next_index(3, 1, Repeat::One, false, true, 0), Some(2), "next skips even on repeat one");
        for pick in 0..50 {
            let n = next_index(5, 2, Repeat::Off, true, false, pick).unwrap();
            assert!(n < 5 && n != 2, "shuffle never repeats the current track");
        }
        assert_eq!(next_index(1, 0, Repeat::Off, true, false, 9), None);
        assert_eq!(next_index(0, 0, Repeat::All, false, false, 0), None);
        assert_eq!(prev_index(2, 10.0, 3, Repeat::Off), (2, true), "restart the track");
        assert_eq!(prev_index(2, 1.0, 3, Repeat::Off), (1, false));
        assert_eq!(prev_index(0, 1.0, 3, Repeat::All), (2, false));
        assert_eq!(prev_index(0, 1.0, 3, Repeat::Off), (0, true));
    }

    #[test]
    fn music_requests_are_understood_and_others_are_not() {
        use MusicCommand::*;
        let p = |s: &str| parse_command(s);
        assert_eq!(p("Groovy, play my favourite songs."), Some(PlayFavourites));
        assert_eq!(p("Play my favorite songs"), Some(PlayFavourites));
        assert_eq!(p("Pause the music"), Some(Pause));
        assert_eq!(p("hey groovy, next song"), Some(Next));
        assert_eq!(p("Skip this song!"), Some(Next));
        assert_eq!(p("Lower the volume"), Some(VolumeDown));
        assert_eq!(p("turn it up please"), Some(VolumeUp));
        assert_eq!(p("set the volume to 40%"), Some(Volume(40)));
        assert_eq!(p("Play the playlist I made yesterday"), Some(PlayPlaylist(PlaylistRef::Latest)));
        assert_eq!(p("play my Road Trip playlist"), Some(PlayPlaylist(PlaylistRef::Named("road trip".into()))));
        assert_eq!(p("play the playlist called \"Focus\""), Some(PlayPlaylist(PlaylistRef::Named("focus".into()))));
        assert_eq!(p("Play some relaxing music"), Some(PlayRelaxing));
        assert_eq!(p("play something chill"), Some(PlayRelaxing));
        assert_eq!(p("play some Daft Punk"), Some(PlaySearch("daft punk".into())));
        assert_eq!(p("play music"), Some(PlayAll));
        assert_eq!(p("resume the music"), Some(Resume));
        // Not music.
        assert_eq!(p("play a game with me"), None);
        assert_eq!(p("What is the capital of France?"), None);
        assert_eq!(p("Can you explain how pause works in JavaScript?"), None);
        assert_eq!(p("next, tell me a joke"), None);
        assert_eq!(p("play with me"), None);
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_lite::encode(b""), "");
        assert_eq!(base64_lite::encode(b"f"), "Zg==");
        assert_eq!(base64_lite::encode(b"fo"), "Zm8=");
        assert_eq!(base64_lite::encode(b"foobar"), "Zm9vYmFy");
    }
}
