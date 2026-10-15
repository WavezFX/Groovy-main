// The microphone, as 16 kHz mono frames for the VoiceController. Opened only
// while voice is on in Settings, and closed as soon as it is turned off.

import { SAMPLE_RATE } from "./controller";

export interface Mic {
  stop(): void;
}

/** Opens the default microphone; `onFrame` gets frames of `frameSize` samples. */
export async function openMic(onFrame: (frame: Float32Array) => void, frameSize = 1024): Promise<Mic> {
  const stream = await navigator.mediaDevices.getUserMedia({
    audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true },
  });
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
