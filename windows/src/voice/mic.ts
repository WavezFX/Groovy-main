// The microphone, as 16 kHz mono frames for the VoiceController. Opened only
// while voice is on in Settings, and closed as soon as it is turned off.

import { SAMPLE_RATE } from "./controller";
import { Bridge, IS_TAURI, onEvent } from "../core/bridge";

export interface Mic {
  stop(): void;
}

/** The constraints for `deviceId` ("" = the system default), noise suppression on. */
export function micConstraints(deviceId = ""): MediaStreamConstraints {
  const audio: MediaTrackConstraints = { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true };
  if (deviceId) audio.deviceId = { exact: deviceId };
  return { audio };
}

/**
 * Opens the microphone picked in Settings (or the default one; a picked one
 * that is gone falls back to the default); `onFrame` gets frames of
 * `frameSize` samples, `onLost` is called once if the device goes away.
 */
export async function openMic(
  onFrame: (frame: Float32Array) => void,
  frameSize = 1024,
  deviceId = "",
  onLost?: () => void,
): Promise<Mic> {
  let stream: MediaStream;
  try {
    stream = await navigator.mediaDevices.getUserMedia(micConstraints(deviceId));
  } catch (err) {
    if (!deviceId || (err as DOMException)?.name !== "OverconstrainedError") throw err;
    stream = await navigator.mediaDevices.getUserMedia(micConstraints(""));
  }
  let stopped = false;
  for (const track of stream.getAudioTracks()) {
    track.addEventListener("ended", () => {
      if (!stopped) onLost?.();
    });
  }
  // The browser resamples the microphone to the context's rate.
  const ctx = new AudioContext({ sampleRate: SAMPLE_RATE });
  const source = ctx.createMediaStreamSource(stream);
  // ScriptProcessor is old but everywhere (WebView2 and WebKitGTK alike) and
  // needs no separate worklet file; at 1024 samples it runs ~16 times a second.
  const node = ctx.createScriptProcessor(frameSize, 1, 1);
  node.onaudioprocess = (e) => {
    onFrame(new Float32Array(e.inputBuffer.getChannelData(0)));
  };
  // A muted gain keeps the node pulling audio without playing the mic back.
  const mute = ctx.createGain();
  mute.gain.value = 0;
  source.connect(node);
  node.connect(mute);
  mute.connect(ctx.destination);
  if (ctx.state === "suspended") await ctx.resume();

  return {
    stop() {
      stopped = true;
      node.onaudioprocess = null;
      try {
        source.disconnect();
        node.disconnect();
        mute.disconnect();
      } catch {
        // already disconnected
      }
      for (const t of stream.getTracks()) t.stop();
      void ctx.close();
    },
  };
}

/**
 * Round 4A item 2: the microphone captured in Rust (the same capture dictation and
 * « Hey Groovy » use), its 16 kHz frames sent to the page. No getUserMedia and no
 * AudioContext in the island, so WebView2's capture pipeline stays down during a
 * call. Falls back to the page's own microphone when Rust can't open one.
 */
export async function openRustMic(
  onFrame: (frame: Float32Array) => void,
  frameSize = 1024,
  deviceId = "",
  onLost?: () => void,
): Promise<Mic> {
  if (!IS_TAURI) return openMic(onFrame, frameSize, deviceId, onLost);
  let stopped = false;
  const unlisten = await onEvent<number[]>("call-mic-frame", (samples) => {
    if (stopped) return;
    const frame = new Float32Array(samples.length);
    for (let i = 0; i < samples.length; i++) frame[i] = samples[i] / 32768;
    onFrame(frame);
  });
  try {
    await Bridge.callMicStart(frameSize);
  } catch {
    unlisten();
    return openMic(onFrame, frameSize, deviceId, onLost);
  }
  return {
    stop() {
      if (stopped) return;
      stopped = true;
      unlisten();
      void Bridge.callMicStop();
    },
  };
}
