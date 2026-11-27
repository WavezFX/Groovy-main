// LiveKit mode: after the wake word, Groovy joins a room on your own LiveKit
// server and talks with the voice agent you run there (LiveKit Agents), live,
// both ways. The access token is made by Rust from the key and secret stored in
// the system keychain (livekit.rs); nothing else leaves this computer.
//
// The conversation ends after a stretch of silence, or when the island asks.

import { Bridge } from "../core/bridge";
import { isEndPhrase } from "./wake";
import type { Participant, RemoteTrack, TranscriptionSegment } from "livekit-client";

/** No one speaking for this long ends the conversation. */
const IDLE_SECONDS = 20;

export interface LiveKitCallbacks {
  /** Connected and the microphone is live. */
  onLive(): void;
  /** What the agent (or you) said, when the agent sends transcriptions. */
  onCaption?(who: "you" | "groovy", text: string): void;
  /** The conversation ended; `error` says why when it never started. */
  onEnd(error?: string): void;
}

export interface LiveKitSession {
  hangUp(): void;
}

export async function startLiveKit(
  cb: LiveKitCallbacks,
  load: () => Promise<typeof import("livekit-client")> = () => import("livekit-client"),
  mint: () => Promise<{ url: string; token: string }> = () => Bridge.livekitToken(),
): Promise<LiveKitSession> {
  let lk: typeof import("livekit-client");
  try {
    lk = await load();
  } catch {
    cb.onEnd("LiveKit isn't part of this build.");
    return { hangUp() {} };
  }
  const access = await mint().catch((e: unknown) => {
    cb.onEnd(String(e).replace(/^Error:\s*/, ""));
    return null;
  });
  if (!access) return { hangUp() {} };

  const room = new lk.Room({ adaptiveStream: true, dynacast: true });
  const players: HTMLMediaElement[] = [];
  let ended = false;
  let idle = 0;

  const end = (error?: string) => {
    if (ended) return;
    ended = true;
    window.clearInterval(idle);
    for (const el of players) el.remove();
    void room.disconnect();
    cb.onEnd(error);
  };

  let lastVoice = performance.now();
  room.on(lk.RoomEvent.ActiveSpeakersChanged, (speakers: Participant[]) => {
    if (speakers.length > 0) lastVoice = performance.now();
  });
  room.on(lk.RoomEvent.TrackSubscribed, (track: RemoteTrack) => {
    if (track.kind !== lk.Track.Kind.Audio) return;
    const el = track.attach();
    el.style.display = "none";
    document.body.append(el);
    players.push(el);
  });
  room.on(lk.RoomEvent.TranscriptionReceived, (segments: TranscriptionSegment[], participant?: Participant) => {
    const text = segments.map((s) => s.text).join(" ").trim();
    if (!text) return;
    lastVoice = performance.now();
    const you = participant?.identity === room.localParticipant.identity;
    cb.onCaption?.(you ? "you" : "groovy", text);
    // Round 4B item 5: "okay bye" leaves the room first, like the hang-up button.
    if (you && segments.every((s) => s.final !== false) && isEndPhrase(text)) end();
  });
  room.on(lk.RoomEvent.Disconnected, () => end());

  try {
    await room.connect(access.url, access.token);
    await room.localParticipant.setMicrophoneEnabled(true);
  } catch (e) {
    end(`Couldn't join LiveKit: ${String(e).replace(/^Error:\s*/, "")}`);
    return { hangUp() {} };
  }
  cb.onLive();
  idle = window.setInterval(() => {
    if (performance.now() - lastVoice > IDLE_SECONDS * 1000) end();
  }, 1000);

  return { hangUp: () => end() };
}

/** The hosts the island's CSP lets LiveKit reach, fixed at build time (round 4B item 5). */
export function livekitHostAllowed(url: string): boolean {
  try {
    const u = new URL(url.trim());
    return u.protocol === "wss:" && (u.hostname === "livekit.cloud" || u.hostname.endsWith(".livekit.cloud"));
  } catch {
    return false;
  }
}

/**
 * Settings → "Test connection": mints a token, joins the room for `holdMs`
 * and leaves. Resolves to null when it worked, else what went wrong.
 * `load` is the lazy import, replaceable in tests.
 */
export async function testLiveKit(
  holdMs = 3000,
  load: () => Promise<typeof import("livekit-client")> = () => import("livekit-client"),
  mint: () => Promise<{ url: string; token: string }> = () => Bridge.livekitToken(),
): Promise<string | null> {
  let access: { url: string; token: string };
  try {
    access = await mint();
  } catch (e) {
    return String(e).replace(/^Error:\s*/, "");
  }
  let lk: typeof import("livekit-client");
  try {
    lk = await load();
  } catch {
    return "LiveKit isn't part of this build.";
  }
  const room = new lk.Room();
  try {
    await room.connect(access.url, access.token);
    await new Promise((r) => setTimeout(r, holdMs));
    return null;
  } catch (e) {
    return `Couldn't join LiveKit: ${String(e).replace(/^Error:\s*/, "")}`;
  } finally {
    await room.disconnect();
  }
}
