# Models Groovy downloads or bundles

Nothing here ships in the installer: each model is downloaded only when the user asks (Settings → Voice / Models), from the source below. Licences are recorded from each model's own page; **entries marked "check" were not re-read for this file and need the owner's eyes before a release that relies on them.**

| Model | Used for | Source | Licence | Credit required? |
|---|---|---|---|---|
| Whisper tiny / base / small (sherpa-onnx exports, `sherpa-onnx-whisper-*`) | Dictation | OpenAI Whisper, exported by k2-fsa | MIT | Keep the notice |
| NVIDIA Parakeet TDT 0.6b v3 int8 (`sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8`) | Dictation (multilingual) | NVIDIA NeMo, exported by k2-fsa | CC-BY-4.0 | **Yes: credit NVIDIA** (check) |
| Zipformer keyword spotter (`sherpa-onnx-kws-zipformer-gigaspeech-3.3M`) | « OK Groovy » wake word | k2-fsa | Apache-2.0 (check) | Keep the notice |
| Pyannote segmentation 3.0 (`sherpa-onnx-pyannote-segmentation-3-0`) | Speech detection | pyannote, exported by k2-fsa | MIT (check) | Keep the notice |
| Kokoro int8 multi-lang v1.0 (`csukuangfj/kokoro-int8-multi-lang-v1_0`) | Offline voice | hexgrad Kokoro-82M, exported by csukuangfj | Apache-2.0 | Keep the notice |
| espeak-ng data (inside the Kokoro download) | Pronunciation for Kokoro | espeak-ng | **GPL-3.0 (data files)**: downloaded at the user's request, never shipped by Groovy. **Owner to confirm this is acceptable** | n/a |
| Local chat models (Qwen, via Ollama or the built-in runtime) | Offline chat | Alibaba / Ollama library | Varies by model; shown where it is downloaded | Per model |

No VAD or turn-detection model is bundled. If one is adopted (round 3J item 2) its row is added here first.
