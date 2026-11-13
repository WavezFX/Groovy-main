// LiveKit mode: after the wake word, Groovy joins a room on your own LiveKit
// server and talks with the voice agent you run there (LiveKit Agents), live,
// both ways. The access token is made by Rust from the key and secret stored in
// the system keychain (livekit.rs); nothing else leaves this computer.
//
// The conversation ends after a stretch of silence, or when the island asks.

import { Bridge } from "../core/bridge";
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

export async function startLiveKit(cb: LiveKitCallbacks): Promise<LiveKitSession> {
  let lk: typeof import("livekit-client");
  try {
    lk = await import("livekit-client");
  } catch {
    cb.onEnd("LiveKit isn't part of this build.");
    return { hangUp() {} };
  }
  const access = await Bridge.livekitToken().catch((e: unknown) => {
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
