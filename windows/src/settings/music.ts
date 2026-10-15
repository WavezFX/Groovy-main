// Settings → Music: Groovy's own player (music.rs). Music folders, the
// library with search and favourites, playlists, and the player itself.
// Everything plays from files on this computer, with no network.

import {
  Bridge, onEvent, type MusicPlaylist, type MusicState, type MusicTrack, type RepeatMode,
} from "../core/bridge";
import { clear, h, svg } from "../views/dom";
import { ICONS } from "../views/icons";
import { t } from "../i18n/i18n";
import { confirmClick } from "./parts";

/** 3:07, 1:02:03 */
export function clock(seconds: number): string {
  const s = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0));
  const hh = Math.floor(s / 3600);
  const mm = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, "0");
  return hh > 0 ? `${hh}:${String(mm).padStart(2, "0")}:${ss}` : `${mm}:${ss}`;
}

const NEXT_REPEAT: Record<RepeatMode, RepeatMode> = { off: "all", all: "one", one: "off" };
const MAX_ROWS = 300;

export function musicSection(): HTMLElement {
  const message = h("div", {});
  const say = (text: string, kind: "ok" | "err" | "warn" = "ok") => {
    clear(message);
    message.append(h("div", { class: `notice ${kind}`, text }));
  };
  const err = (e: unknown) => say(String(e).replace(/^Error:\s*/, ""), "err");

  // ── Player ──
  const art = h("div", { class: "music-art" });
  const title = h("div", { class: "music-title" });
  const artist = h("div", { class: "hint" });
  const iconBtn = (path: string, label: string, onClick: () => void, stroke = false) => {
    const b = h("button", { class: "icon-btn", title: label, "aria-label": label }, svg(path, 14, stroke ? { stroke: 1.8 } : {})) as HTMLButtonElement;
    b.addEventListener("click", onClick);
    return b;
  };
  let state: MusicState | null = null;
  const prev = iconBtn(ICONS.backward, t("Previous"), () => void Bridge.musicControl("previous"));
  const playBtn = iconBtn(ICONS.play, t("Play"), () => void Bridge.musicControl("toggle"));
  const next = iconBtn(ICONS.forward, t("Next"), () => void Bridge.musicControl("next"));
  const shuffle = iconBtn(ICONS.shuffle, t("Shuffle"), () => state && void Bridge.musicSetMode(!state.shuffle, state.repeat), true);
  const repeat = iconBtn(ICONS.repeat, t("Repeat"), () => state && void Bridge.musicSetMode(state.shuffle, NEXT_REPEAT[state.repeat]), true);
  const repeatOne = h("span", { class: "repeat-one", text: "1" });
  repeat.append(repeatOne);
  const seek = h("input", { type: "range", min: 0, max: 1, step: 0.5, value: 0, class: "music-seek", "aria-label": t("Position") }) as HTMLInputElement;
  let seeking = false;
  seek.addEventListener("input", () => (seeking = true));
  seek.addEventListener("change", () => {
    seeking = false;
    void Bridge.musicSeek(Number(seek.value));
  });
  const time = h("span", { class: "hint music-time" });
  const volume = h("input", { type: "range", min: 0, max: 1, step: 0.01, value: 0.7, "aria-label": t("Volume"), style: "width:90px" }) as HTMLInputElement;
  volume.addEventListener("input", () => void Bridge.musicVolume(Number(volume.value)));
  const playerError = h("div", { class: "hint warn" });
  let artFor: number | null = null;
  let tickFrom = 0;
  let tickAt = 0;

  const drawPlayer = (s: MusicState) => {
    state = s;
    const tr = s.track;
    title.textContent = tr ? tr.title : t("Nothing playing");
    artist.textContent = tr ? [tr.artist, tr.album].filter(Boolean).join(" · ") : t("Pick a track below, or ask Groovy to play something.");
    clear(playBtn);
    playBtn.append(svg(s.playing ? ICONS.pause : ICONS.play, 14));
    playBtn.title = s.playing ? t("Pause") : t("Play");
    playBtn.disabled = s.queue.length === 0;
    prev.disabled = next.disabled = s.queue.length === 0;
    shuffle.classList.toggle("on", s.shuffle);
    shuffle.setAttribute("aria-pressed", String(s.shuffle));
    repeat.classList.toggle("on", s.repeat !== "off");
    repeatOne.style.display = s.repeat === "one" ? "" : "none";
    seek.max = String(Math.max(1, s.duration));
    if (!seeking) seek.value = String(s.position);
    time.textContent = `${clock(s.position)} / ${clock(s.duration)}`;
    if (document.activeElement !== volume) volume.value = String(s.volume);
    playerError.textContent = s.error ?? "";
    tickFrom = s.position;
    tickAt = performance.now();
    if (tr?.id !== artFor) {
      artFor = tr?.id ?? null;
      clear(art);
      art.append(svg(ICONS.musicNote, 22));
      if (tr?.hasArt) {
        void Bridge.musicArt(tr.id).then((url) => {
          if (!url || artFor !== tr.id) return;
          clear(art);
          art.append(h("img", { src: url, alt: "" }));
        });
      }
    }
    for (const row of list.querySelectorAll<HTMLElement>(".track-row")) {
      row.classList.toggle("is-playing", tr != null && row.dataset.id === String(tr.id));
    }
  };
  // The position runs on between events while playing.
  const tick = () => {
    if (!section.isConnected) return;
    if (state?.playing && !seeking) {
      const pos = Math.min(state.duration || Infinity, tickFrom + (performance.now() - tickAt) / 1000);
      seek.value = String(pos);
      time.textContent = `${clock(pos)} / ${clock(state.duration)}`;
    }
    setTimeout(tick, 500);
  };

  // ── Library ──
  const folderList = h("div", { class: "stack" });
  const search = h("input", { type: "search", placeholder: t("Search title, artist, album or genre"), style: "flex:1", "aria-label": t("Search music") }) as HTMLInputElement;
  const favOnly = h("button", { text: t("Favourites only"), "aria-pressed": false }) as HTMLButtonElement;
  let favourites = false;
  const list = h("div", { class: "track-list", role: "list" });
  const count = h("span", { class: "hint" });
  let shown: MusicTrack[] = [];
  let playlistCache: MusicPlaylist[] = [];

  function trackRow(tr: MusicTrack, index: number, queue: () => number[], extra?: HTMLElement): HTMLElement {
    const star = h("button", { class: tr.favourite ? "fav on" : "fav", text: tr.favourite ? "★" : "☆", title: tr.favourite ? t("Remove from favourites") : t("Add to favourites"), "aria-pressed": tr.favourite }) as HTMLButtonElement;
    star.addEventListener("click", async (e) => {
      e.stopPropagation();
      try {
        await Bridge.musicSetFavourite(tr.id, !tr.favourite);
        tr.favourite = !tr.favourite;
        star.textContent = tr.favourite ? "★" : "☆";
        star.classList.toggle("on", tr.favourite);
      } catch (e2) {
        err(e2);
      }
    });
    const add = h("select", { class: "add-to", "aria-label": t("Add to a playlist") }) as HTMLSelectElement;
    add.append(h("option", { value: "", text: "+" }));
    for (const p of playlistCache) add.append(h("option", { value: String(p.id), text: p.name }));
    add.addEventListener("click", (e) => e.stopPropagation());
    add.addEventListener("change", async () => {
      const id = Number(add.value);
      add.value = "";
      if (!id) return;
      try {
        await Bridge.musicPlaylistAdd(id, [tr.id]);
        say(t("Added to “{name}”.", { name: playlistCache.find((p) => p.id === id)?.name ?? "" }));
        await drawPlaylists();
      } catch (e2) {
        err(e2);
      }
    });
    const row = h(
      "div",
      { class: "track-row", role: "listitem", tabindex: 0, "data-id": String(tr.id), title: t("Double-click to play") },
      h("div", { class: "name" }, h("div", { text: tr.title }), h("div", { class: "hint", text: [tr.artist, tr.album].filter(Boolean).join(" · ") })),
      h("span", { class: "hint", text: clock(tr.duration) }),
      star,
      extra ?? add,
    );
    const play = () => void Bridge.musicPlay(queue(), index).catch(err);
    row.addEventListener("dblclick", play);
    row.addEventListener("keydown", (e) => {
      if (e.key === "Enter") play();
    });
    if (state?.track?.id === tr.id) row.classList.add("is-playing");
    return row;
  }

  async function drawLibrary() {
    try {
      shown = await Bridge.musicSearch(search.value.trim(), favourites);
    } catch (e) {
      clear(list);
      list.append(h("div", { class: "hint", text: String(e).replace(/^Error:\s*/, "") }));
      return;
    }
    clear(list);
    count.textContent = t("{count} tracks", { count: shown.length });
    if (shown.length === 0) {
      list.append(h("div", { class: "hint", text: search.value || favourites ? t("No track matches.") : t("No music yet: add a folder above.") }));
    }
    const ids = shown.map((x) => x.id);
    shown.slice(0, MAX_ROWS).forEach((tr, i) => list.append(trackRow(tr, i, () => ids)));
    if (shown.length > MAX_ROWS) list.append(h("div", { class: "hint", text: t("Showing the first {n}: search to narrow it down.", { n: MAX_ROWS }) }));
  }

  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  search.addEventListener("input", () => {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => void drawLibrary(), 200);
  });
  favOnly.addEventListener("click", () => {
    favourites = !favourites;
    favOnly.classList.toggle("primary", favourites);
    favOnly.setAttribute("aria-pressed", String(favourites));
    void drawLibrary();
  });
  const playAll = h("button", { class: "primary", text: t("Play all") }) as HTMLButtonElement;
  playAll.addEventListener("click", () => shown.length && void Bridge.musicPlay(shown.map((x) => x.id), 0).catch(err));

  async function drawFolders() {
    clear(folderList);
    try {
      const folders = await Bridge.musicFolders();
      if (folders.length === 0) folderList.append(h("div", { class: "hint", text: t("Add the folders where your music is. MP3, FLAC, Ogg, WAV and M4A files are played.") }));
      for (const f of folders) {
        const del = h("button", { class: "danger", text: t("Remove") }) as HTMLButtonElement;
        confirmClick(del, t("Click again: files stay on disk"), async () => {
          await Bridge.musicRemoveFolder(f.id).catch(err);
          await drawFolders();
          await drawLibrary();
        });
        folderList.append(
          h("div", { class: "pill-row full" },
            h("div", { class: "name" }, h("div", { class: "path", text: f.path }), h("div", { class: f.exists ? "hint" : "hint warn", text: f.exists ? t("{count} tracks", { count: f.tracks }) : t("Folder not found") })),
            del),
        );
      }
    } catch (e) {
      folderList.append(h("div", { class: "hint", text: String(e).replace(/^Error:\s*/, "") }));
    }
  }
  const addFolder = h("button", { class: "primary", text: t("Add a folder…") }) as HTMLButtonElement;
  const rescan = h("button", { text: t("Rescan") }) as HTMLButtonElement;
  const report = (r: { found: number; added: number; updated: number; removed: number }) =>
    say(t("{found} files: {added} new, {updated} changed, {removed} gone.", r));
  addFolder.addEventListener("click", async () => {
    addFolder.disabled = true;
    try {
      const r = await Bridge.musicAddFolder();
      if (r) report(r);
      await drawFolders();
      await drawLibrary();
    } catch (e) {
      err(e);
    } finally {
      addFolder.disabled = false;
    }
  });
  rescan.addEventListener("click", async () => {
    rescan.disabled = true;
    try {
      report(await Bridge.musicRescan());
      await drawFolders();
      await drawLibrary();
    } catch (e) {
      err(e);
    } finally {
      rescan.disabled = false;
    }
  });
  const scanProgress = h("span", { class: "hint" });
  void onEvent<{ done: number; total: number }>("music-scan", (p) => {
    scanProgress.textContent = p.done >= p.total ? "" : t("Scanning… {done} of {total}", p);
  });

  // ── Playlists ──
  const playlistList = h("div", { class: "stack" });
  const newName = h("input", { type: "text", maxlength: 80, placeholder: t("New playlist"), style: "flex:1" }) as HTMLInputElement;
  const create = h("button", { text: t("Create") }) as HTMLButtonElement;
  const createPlaylist = async () => {
    if (!newName.value.trim()) return;
    try {
      await Bridge.musicPlaylistCreate(newName.value);
      newName.value = "";
      await drawPlaylists();
      await drawLibrary();
    } catch (e) {
      err(e);
    }
  };
  create.addEventListener("click", () => void createPlaylist());
  newName.addEventListener("keydown", (e) => {
    if (e.key === "Enter") void createPlaylist();
  });
  const open = new Set<number>();

  async function drawPlaylists() {
    try {
      playlistCache = await Bridge.musicPlaylists();
    } catch {
      playlistCache = [];
    }
    clear(playlistList);
    if (playlistCache.length === 0) playlistList.append(h("div", { class: "hint", text: t("No playlists yet.") }));
    for (const p of playlistCache) {
      const items = h("div", { class: "track-list nested" });
      const play = h("button", { class: "primary", text: t("Play") }) as HTMLButtonElement;
      play.disabled = p.tracks === 0;
      play.addEventListener("click", async () => {
        const tracks = await Bridge.musicPlaylistTracks(p.id).catch(() => []);
        if (tracks.length) await Bridge.musicPlay(tracks.map((x) => x.id), 0).catch(err);
      });
      const toggle = h("button", { text: open.has(p.id) ? t("Hide") : t("Show") }) as HTMLButtonElement;
      const fill = async () => {
        clear(items);
        const tracks = await Bridge.musicPlaylistTracks(p.id).catch(() => []);
        const ids = tracks.map((x) => x.id);
        tracks.forEach((tr, i) => {
          const remove = h("button", { class: "icon-btn", title: t("Remove from the playlist"), "aria-label": t("Remove from the playlist") }, svg(ICONS.xmark, 10)) as HTMLButtonElement;
          remove.addEventListener("click", async (e) => {
            e.stopPropagation();
            await Bridge.musicPlaylistRemove(p.id, i).catch(err);
            await drawPlaylists();
          });
          items.append(trackRow(tr, i, () => ids, remove));
        });
      };
      toggle.addEventListener("click", async () => {
        if (open.has(p.id)) {
          open.delete(p.id);
          clear(items);
          toggle.textContent = t("Show");
        } else {
          open.add(p.id);
          toggle.textContent = t("Hide");
          await fill();
        }
      });
      if (open.has(p.id)) void fill();
      const del = h("button", { class: "danger", text: t("Delete") }) as HTMLButtonElement;
      confirmClick(del, t("Click again to delete"), async () => {
        await Bridge.musicPlaylistDelete(p.id).catch(err);
        await drawPlaylists();
        await drawLibrary();
      });
      playlistList.append(
        h("div", { class: "pill-row full" }, h("div", { class: "name" }, h("div", { text: p.name }), h("div", { class: "hint", text: t("{count} tracks", { count: p.tracks }) })), play, toggle, del),
        items,
      );
    }
  }

  const section = h(
    "section",
    { id: "music" },
    h("h2", {}, h("span", { text: t("Music") })),
    message,
    h(
      "div",
      { class: "music-player" },
      art,
      h("div", { class: "music-info" }, title, artist, h("div", { class: "row music-seek-row" }, seek, time), playerError),
      h("div", { class: "music-controls" }, h("div", { class: "row" }, shuffle, prev, playBtn, next, repeat), h("div", { class: "row" }, svg(ICONS.speakerOn, 12), volume)),
    ),
    h("div", { class: "hint", text: t("Plays files on this computer, with no internet. Ask Groovy: “play my favourite songs”, “next song”, “lower the volume”, “play the playlist I made yesterday”.") }),
    h("div", { class: "subhead", text: t("Music folders") }),
    folderList,
    h("div", { class: "row" }, addFolder, rescan, scanProgress),
    h("div", { class: "subhead", text: t("Playlists") }),
    h("div", { class: "row" }, newName, create),
    playlistList,
    h("div", { class: "subhead", text: t("Library") }),
    h("div", { class: "row" }, search, favOnly, playAll, count),
    list,
  );

  void onEvent<MusicState>("music-state", drawPlayer);
  void (async () => {
    const s = await Bridge.musicState();
    if (s) drawPlayer(s);
    else {
      title.textContent = t("Nothing playing");
      for (const b of [prev, playBtn, next, shuffle, repeat]) b.disabled = true;
    }
    await drawFolders();
    await drawPlaylists();
    await drawLibrary();
    tick();
  })();
  return section;
}
