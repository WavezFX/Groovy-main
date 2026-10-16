// "Instant voice" for Talk to Groovy (round 4 item 3): a real-time voice
// model instead of speech → AI → voice, when the user chose it and has the
// key. OpenAI Realtime runs over WebRTC (the browser carries the audio both
// ways, with its echo cancellation; Rust answers the offer, so no key is ever
// in this page). Gemini Live runs over a WebSocket with a one-use token Rust
// asks for. Either way the model hears you, answers and stops when you talk
// over it; captions come back as text. Any failure falls back to the normal
// pipeline (island/voice.ts).

import { Bridge } from "../core/bridge";

export type InstantProvider = "openai" | "gemini";

export interface InstantHooks {
  /** What you said, or what Groovy is saying, as it comes. */
  onCaption(who: "you" | "groovy", text: string): void;
  /** Groovy started or stopped talking. */
  onSpeaking(speaking: boolean): void;
  /** Finished turns, for the summary. */
  onTurn(who: "user" | "assistant", text: string): void;
  /** It ended: by hanging up (no error), or because it failed. */
  onEnd(error: string | null): void;
}

export interface InstantSession {
  hangUp(): void;
  mute(muted: boolean): void;
}

export async function startInstant(provider: InstantProvider, hooks: InstantHooks): Promise<InstantSession> {
  return provider === "openai" ? startOpenAI(hooks) : startGemini(hooks);
}

// ── OpenAI Realtime (WebRTC) ──────────────────────────────────────────────────

async function startOpenAI(hooks: InstantHooks): Promise<InstantSession> {
  const mic = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true } });
  const pc = new RTCPeerConnection();
  const audio = new Audio();
  audio.autoplay = true;
  pc.ontrack = (e) => {
    audio.srcObject = e.streams[0];
  };
  for (const track of mic.getTracks()) pc.addTrack(track, mic);
  const events = pc.createDataChannel("oai-events");
  let ended = false;
  let you = "";
  let groovy = "";
  const end = (error: string | null) => {
    if (ended) return;
    ended = true;
    events.close();
    pc.close();
    for (const t of mic.getTracks()) t.stop();
    audio.srcObject = null;
    hooks.onEnd(error);
  };
  events.onmessage = (m) => {
    let e: { type?: string; delta?: string; transcript?: string; error?: { message?: string } };
    try {
      e = JSON.parse(String(m.data));
    } catch {
      return;
    }
    switch (e.type) {
      case "input_audio_buffer.speech_started":
        hooks.onSpeaking(false);
        you = "";
        break;
      case "conversation.item.input_audio_transcription.delta":
        you += e.delta ?? "";
        hooks.onCaption("you", you);
        break;
      case "conversation.item.input_audio_transcription.completed":
        if (e.transcript) hooks.onTurn("user", e.transcript);
        break;
      case "response.output_audio_transcript.delta":
        if (!groovy) hooks.onSpeaking(true);
        groovy += e.delta ?? "";
        hooks.onCaption("groovy", groovy);
        break;
      case "response.output_audio_transcript.done":
        if (e.transcript) hooks.onTurn("assistant", e.transcript);
        groovy = "";
        break;
      case "response.done":
        hooks.onSpeaking(false);
        break;
      case "error":
        end(e.error?.message ?? "OpenAI Realtime failed");
        break;
    }
  };
  pc.onconnectionstatechange = () => {
    if (pc.connectionState === "failed") end("The connection to OpenAI was lost.");
  };
  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);
  try {
    const sdp = await Bridge.instantOpenaiAnswer(offer.sdp ?? "");
    await pc.setRemoteDescription({ type: "answer", sdp });
  } catch (err) {
    end(String(err).replace(/^Error:\s*/, ""));
    throw err;
  }
  return {
    hangUp: () => end(null),
    mute: (muted) => mic.getAudioTracks().forEach((t) => (t.enabled = !muted)),
  };
}

// ── Gemini Live (WebSocket) ───────────────────────────────────────────────────

const GEMINI_WS =
  "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateContentConstrained";

async function startGemini(hooks: InstantHooks): Promise<InstantSession> {
  const { token, setup } = await Bridge.instantGeminiToken();
  const mic = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, channelCount: 1 } });
  const ctx = new AudioContext({ sampleRate: 16000 });
  const out = new AudioContext({ sampleRate: 24000 });
  const source = ctx.createMediaStreamSource(mic);
  // ScriptProcessor is deprecated but needs no separate worklet file: 4096
  // samples at 16 kHz is a quarter-second chunk, plenty for the model.
  const tap = ctx.createScriptProcessor(4096, 1, 1);
  const ws = new WebSocket(`${GEMINI_WS}?access_token=${encodeURIComponent(token)}`);
  let ended = false;
  let muted = false;
  let ready = false;
  let playAt = 0;
  const playing: AudioBufferSourceNode[] = [];
  let you = "";
  let groovy = "";
  const stopPlayback = () => {
    for (const p of playing.splice(0)) {
      try {
        p.stop();
      } catch {
        /* already done */
      }
    }
    playAt = 0;
    hooks.onSpeaking(false);
  };
  const end = (error: string | null) => {
    if (ended) return;
    ended = true;
    tap.disconnect();
    source.disconnect();
    for (const t of mic.getTracks()) t.stop();
    stopPlayback();
    void ctx.close();
    void out.close();
    if (ws.readyState === WebSocket.OPEN) ws.close();
    hooks.onEnd(error);
  };
  tap.onaudioprocess = (e) => {
    if (!ready || muted || ws.readyState !== WebSocket.OPEN) return;
    const pcm = e.inputBuffer.getChannelData(0);
    ws.send(JSON.stringify({ realtimeInput: { audio: { data: toBase64Pcm16(pcm), mimeType: "audio/pcm;rate=16000" } } }));
  };
  source.connect(tap);
  tap.connect(ctx.destination);
  ws.onopen = () => ws.send(JSON.stringify(setup));
  ws.onerror = () => end("The connection to Gemini failed.");
  ws.onclose = (e) => end(e.code === 1000 ? null : e.reason || "Gemini closed the conversation.");
  ws.onmessage = async (m) => {
    const text = typeof m.data === "string" ? m.data : await (m.data as Blob).text();
    let msg: GeminiMessage;
    try {
      msg = JSON.parse(text);
    } catch {
      return;
    }
    if (msg.setupComplete) ready = true;
    const c = msg.serverContent;
    if (!c) return;
    if (c.interrupted) {
      // You talked over him: he stops at once.
      stopPlayback();
      if (groovy) hooks.onTurn("assistant", groovy);
      groovy = "";
    }
    if (c.inputTranscription?.text) {
      you += c.inputTranscription.text;
      hooks.onCaption("you", you);
    }
    if (c.outputTranscription?.text) {
      groovy += c.outputTranscription.text;
      hooks.onCaption("groovy", groovy);
    }
    for (const part of c.modelTurn?.parts ?? []) {
      if (!part.inlineData?.data) continue;
      const samples = fromBase64Pcm16(part.inlineData.data);
      const buffer = out.createBuffer(1, samples.length, 24000);
      buffer.copyToChannel(samples, 0);
      const node = out.createBufferSource();
      node.buffer = buffer;
      node.connect(out.destination);
      playAt = Math.max(playAt, out.currentTime);
      node.start(playAt);
      playAt += buffer.duration;
      playing.push(node);
      node.onended = () => {
        playing.splice(playing.indexOf(node), 1);
        if (playing.length === 0) hooks.onSpeaking(false);
      };
      hooks.onSpeaking(true);
    }
    if (c.turnComplete) {
      if (you) hooks.onTurn("user", you);
      if (groovy) hooks.onTurn("assistant", groovy);
      you = "";
      groovy = "";
    }
  };
  return {
    hangUp: () => end(null),
    mute: (m) => (muted = m),
  };
}

interface GeminiMessage {
  setupComplete?: unknown;
  serverContent?: {
    interrupted?: boolean;
    turnComplete?: boolean;
    inputTranscription?: { text?: string };
    outputTranscription?: { text?: string };
    modelTurn?: { parts?: { inlineData?: { data?: string } }[] };
  };
}

/** -1…1 floats → little-endian 16-bit PCM, base64. */
export function toBase64Pcm16(pcm: Float32Array): string {
  const bytes = new Uint8Array(pcm.length * 2);
  const view = new DataView(bytes.buffer);
  for (let i = 0; i < pcm.length; i++) {
    const v = Math.max(-1, Math.min(1, pcm[i]));
    view.setInt16(i * 2, Math.round(v * 32767), true);
  }
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

/** Base64 little-endian 16-bit PCM → -1…1 floats. */
export function fromBase64Pcm16(data: string): Float32Array<ArrayBuffer> {
  const raw = atob(data);
  const out = new Float32Array(raw.length >> 1);
  for (let i = 0; i < out.length; i++) {
    const v = raw.charCodeAt(2 * i) | (raw.charCodeAt(2 * i + 1) << 8);
    out[i] = (v >= 0x8000 ? v - 0x10000 : v) / 32768;
  }
  return out;
}
