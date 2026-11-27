# Round 4F item 2: the lightest local voice and speech-to-text that still sound good.
# Runs every candidate in its own process (so none is measured on another's leftovers)
# through sherpa-onnx's Python package, the same engine Groovy's helpers use, and logs:
#   voices: download size, load time, private memory after loading and at most while
#           speaking three replies, time to the first clause, real-time factor (RTF);
#   speech-to-text: download size, private memory, time to transcript, word errors.
# Private memory is what Groovy's Linux meter reads (ceiling.rs): "Anonymous" in
# /proc/self/smaps_rollup. Here the weights are copied into memory; Groovy maps them
# from disk (speech/mapped.rs), so on Windows a voice reads about its weights' size less.
#
#   python3 -I scripts/voice-bench.py <models dir> <clips dir> <out dir>
#
# <out dir> gets one WAV per voice (the same three sentences, the owner listens and picks),
# bench.md (the table) and bench.json. CI: .github/workflows/voice-bench.yml.
import json, os, subprocess, sys, time, wave

sys.path.insert(0, os.environ.get("SHERPA_PY", "."))  # -I ignores PYTHONPATH

TEXTS = [
    "Hi, I'm Groovy. It's a quarter past three, and the report is on its way to Sam.",
    "Your next meeting is with the design team at four, in the small room upstairs.",
    "I paused the music and turned the volume down a little, like you asked.",
]
# Clips with what was said: the repo's test clips, then the reference voice's three replies.
CLIPS = {
    "dictation.wav": "please send the report to sam by friday",
    "hey-groovy-time.wav": "hey groovy what time is it",
    "ok-groovy-time.wav": "ok groovy what time is it",
}

# id, kind, folder, licence, speaker
VOICES = [
    ("piper-libritts_r-medium", "vits", "vits-piper-en_US-libritts_r-medium", "en_US-libritts_r-medium.onnx", "CC BY 4.0 (LibriTTS-R); today's light voice", 38),
    ("piper-lessac-low", "vits", "vits-piper-en_US-lessac-low", "en_US-lessac-low.onnx", "Blizzard 2013 Lessac: research use (check before shipping)", 0),
    ("piper-amy-low", "vits", "vits-piper-en_US-amy-low", "en_US-amy-low.onnx", "mimic3-voices dataset licence (check before shipping)", 0),
    ("piper-alba-medium", "vits", "vits-piper-en_GB-alba-medium", "en_GB-alba-medium.onnx", "CC BY 4.0 (Alba, Scottish English)", 0),
    ("kitten-nano-v0_2", "kitten", "kitten-nano-en-v0_2-fp16", "model.fp16.onnx", "Apache-2.0 (KittenML)", 0),
    ("matcha-ljspeech", "matcha", "matcha-icefall-en_US-ljspeech", "model-steps-3.onnx", "Apache-2.0 model, LJSpeech public domain; vocos MIT", 0),
]
RECOGNISERS = [
    ("whisper-base.en", "whisper", "sherpa-onnx-whisper-base.en", "base.en", "MIT; today's model is Whisper base"),
    ("whisper-tiny.en", "whisper", "sherpa-onnx-whisper-tiny.en", "tiny.en", "MIT"),
    ("moonshine-tiny-int8", "moonshine", "sherpa-onnx-moonshine-tiny-en-int8", "", "MIT"),
    ("moonshine-base-int8", "moonshine", "sherpa-onnx-moonshine-base-en-int8", "", "MIT"),
]


def private_mb():
    with open("/proc/self/smaps_rollup") as f:
        return sum(int(l.split()[1]) for l in f if l.startswith("Anonymous:")) / 1024


def size_mb(paths):
    total = 0
    for p in paths:
        if os.path.isdir(p):
            for root, _, files in os.walk(p):
                total += sum(os.path.getsize(os.path.join(root, f)) for f in files)
        elif os.path.exists(p):
            total += os.path.getsize(p)
    return total / 1e6


def write_wav(path, samples, rate):
    import numpy as np
    pcm = (np.clip(np.asarray(samples, dtype=np.float32), -1, 1) * 32767).astype("<i2")
    with wave.open(path, "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(rate); w.writeframes(pcm.tobytes())


def run_voice(models, out, threads, row):
    import numpy as np, sherpa_onnx as so
    vid, kind, folder, model, _, sid = row
    d = os.path.join(models, folder)
    files = [d]
    if kind == "vits":
        mc = so.OfflineTtsModelConfig(vits=so.OfflineTtsVitsModelConfig(model=f"{d}/{model}", tokens=f"{d}/tokens.txt", data_dir=f"{d}/espeak-ng-data"), num_threads=threads)
    elif kind == "kitten":
        mc = so.OfflineTtsModelConfig(kitten=so.OfflineTtsKittenModelConfig(model=f"{d}/{model}", voices=f"{d}/voices.bin", tokens=f"{d}/tokens.txt", data_dir=f"{d}/espeak-ng-data"), num_threads=threads)
    else:
        voc = os.path.join(models, "vocos-22khz-univ.onnx"); files.append(voc)
        mc = so.OfflineTtsModelConfig(matcha=so.OfflineTtsMatchaModelConfig(acoustic_model=f"{d}/{model}", vocoder=voc, tokens=f"{d}/tokens.txt", data_dir=f"{d}/espeak-ng-data"), num_threads=threads)
    before = private_mb()
    t = time.time()
    tts = so.OfflineTts(so.OfflineTtsConfig(model=mc, max_num_sentences=1))
    load_s = time.time() - t
    loaded = private_mb()
    peak, first, gen, dur, audio, rate = loaded, None, 0.0, 0.0, [], 0
    for text in TEXTS:
        # Spoken in clauses as Groovy speaks them (tts.rs): the first clause's time is what the user waits for.
        t = time.time()
        a = tts.generate(text, sid=sid, speed=1.0)
        took = time.time() - t
        if first is None:
            c = tts.generate(text.split(",")[0], sid=sid, speed=1.0)
            first = time.time() - t - took
        gen += took
        rate = a.sample_rate
        dur += len(a.samples) / rate
        audio.extend(a.samples); audio.extend([0.0] * int(0.4 * rate))
        peak = max(peak, private_mb())
    write_wav(os.path.join(out, f"voice-{vid}.wav"), np.array(audio), rate)
    return {"id": vid, "download_mb": round(size_mb(files), 1), "load_s": round(load_s, 2), "loaded_mb": round(loaded - before, 1),
            "peak_mb": round(peak - before, 1), "first_clause_s": round(first, 2), "rtf": round(gen / dur, 3), "rate": rate, "threads": threads}


def words(s):
    return "".join(c if c.isalnum() or c == " " else " " for c in s.lower().replace("okay", "ok")).split()


def wer(ref, hyp):
    r, h = words(ref), words(hyp)
    d = list(range(len(h) + 1))
    for i in range(1, len(r) + 1):
        prev, d[0] = d[0], i
        for j in range(1, len(h) + 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (r[i - 1] != h[j - 1]))
    return d[len(h)]


def run_stt(models, clips, threads, row):
    import numpy as np, sherpa_onnx as so
    rid, kind, folder, pre, _ = row
    d = os.path.join(models, folder)
    before = private_mb()
    t = time.time()
    if kind == "whisper":
        rec = so.OfflineRecognizer.from_whisper(encoder=f"{d}/{pre}-encoder.int8.onnx", decoder=f"{d}/{pre}-decoder.int8.onnx", tokens=f"{d}/{pre}-tokens.txt", num_threads=threads, language="en", task="transcribe")
    else:
        rec = so.OfflineRecognizer.from_moonshine(preprocessor=f"{d}/preprocess.onnx", encoder=f"{d}/encode.int8.onnx", uncached_decoder=f"{d}/uncached_decode.int8.onnx", cached_decoder=f"{d}/cached_decode.int8.onnx", tokens=f"{d}/tokens.txt", num_threads=threads)
    load_s = time.time() - t
    loaded = private_mb()
    peak, errors, ref_words, times, heard = loaded, 0, 0, [], {}
    for name, ref in clips:
        with wave.open(name) as w:
            x = np.frombuffer(w.readframes(w.getnframes()), dtype="<i2").astype(np.float32) / 32768
            sr = w.getframerate()
        s = rec.create_stream()
        s.accept_waveform(sr, x)
        t = time.time()
        rec.decode_stream(s)
        times.append(time.time() - t)
        text = s.result.text.strip()
        heard[os.path.basename(name)] = text
        errors += wer(ref, text); ref_words += len(words(ref))
        peak = max(peak, private_mb())
    files = [os.path.join(d, f) for f in os.listdir(d) if f.endswith(".onnx") or f.endswith(".txt")]
    return {"id": rid, "download_mb": round(size_mb(files), 1), "load_s": round(load_s, 2), "loaded_mb": round(loaded - before, 1),
            "peak_mb": round(peak - before, 1), "transcript_s": round(sorted(times)[len(times) // 2], 2), "word_errors": errors,
            "words": ref_words, "heard": heard, "threads": threads}


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "--one":
        # Child: one candidate, its result as JSON on the last line.
        _, _, what, idx, models, clips_json, out, threads = sys.argv
        threads = int(threads)
        if what == "voice":
            r = run_voice(models, out, threads, VOICES[int(idx)])
        else:
            r = run_stt(models, json.loads(clips_json), threads, RECOGNISERS[int(idx)])
        print(json.dumps(r))
        return
    models, clip_dir, out = sys.argv[1:4]
    threads = [int(t) for t in os.environ.get("BENCH_THREADS", "2,1").split(",")]
    os.makedirs(out, exist_ok=True)
    py = [sys.executable, "-I", os.path.abspath(__file__), "--one"]
    env = dict(os.environ)

    def child(what, i, clips, th):
        p = subprocess.run(py + [what, str(i), models, json.dumps(clips), out, str(th)], capture_output=True, text=True, env=env)
        if p.returncode != 0:
            print(f"  {what} {i} failed: {p.stderr.strip().splitlines()[-1:]}", file=sys.stderr)
            return None
        return json.loads(p.stdout.strip().splitlines()[-1])

    voices, stts = [], []
    for i, row in enumerate(VOICES):
        if not os.path.isdir(os.path.join(models, row[2])):
            continue
        for th in threads:
            r = child("voice", i, [], th)
            if r:
                r["licence"] = row[4]; voices.append(r); print("voice", json.dumps(r), flush=True)
    clips = [(os.path.join(clip_dir, n), t) for n, t in CLIPS.items()]
    # The reference voice's replies as three more clips, at 16 kHz like the microphone.
    ref = os.path.join(out, "voice-piper-libritts_r-medium.wav")
    if os.path.exists(ref):
        clips.append((ref, " ".join(TEXTS)))
    for i, row in enumerate(RECOGNISERS):
        if not os.path.isdir(os.path.join(models, row[2])):
            continue
        for th in threads:
            r = child("stt", i, clips, th)
            if r:
                r["licence"] = row[4]; stts.append(r); print("stt", json.dumps(r), flush=True)
    json.dump({"voices": voices, "speech_to_text": stts}, open(os.path.join(out, "bench.json"), "w"), indent=1)
    with open(os.path.join(out, "bench.md"), "w") as f:
        f.write("| Voice | threads | download MB | memory loaded / speaking MB | first clause s | RTF | licence |\n|---|---|---|---|---|---|---|\n")
        for v in voices:
            f.write(f"| {v['id']} | {v['threads']} | {v['download_mb']} | {v['loaded_mb']} / {v['peak_mb']} | {v['first_clause_s']} | {v['rtf']} | {v['licence']} |\n")
        f.write("\n| Speech-to-text | threads | download MB | memory loaded / working MB | transcript s (median) | word errors | licence |\n|---|---|---|---|---|---|---|\n")
        for s in stts:
            f.write(f"| {s['id']} | {s['threads']} | {s['download_mb']} | {s['loaded_mb']} / {s['peak_mb']} | {s['transcript_s']} | {s['word_errors']} of {s['words']} | {s['licence']} |\n")
        f.write("\nWhat each recogniser heard:\n\n")
        for s in stts:
            if s["threads"] == threads[0]:
                f.write(f"- {s['id']}: " + " / ".join(f"{k}: \"{v}\"" for k, v in s["heard"].items()) + "\n")
    print(open(os.path.join(out, "bench.md")).read())


if __name__ == "__main__":
    main()
