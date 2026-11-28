# sherpa-onnx-sys 1.13.8 (vendored, one build-dependency changed)

This is the crates.io release of `sherpa-onnx-sys` 1.13.8 (Apache-2.0, see
`LICENSE`; upstream https://github.com/k2-fsa/sherpa-onnx), used by Groovy
through `[patch.crates-io]` in `windows/Cargo.toml`.

## What was changed

Only `Cargo.toml`: the build-dependency on `zip` moved from `version = "2"`
to `version = "9"`, with `default-features = false, features = ["deflate"]`.
The files `.cargo_vcs_info.json`, `Cargo.toml.orig` and `Cargo.lock` from the
published crate were left out, and the upstream README was replaced by this
one. `build.rs` and `src/` are byte-for-byte the published 1.13.8.

## Why

zip 2.4.2 carries RUSTSEC-2026-0367/0368/0369/0429/0545/0548 (and, through
its zstd feature, zstd 0.13 / zstd-safe 7 advisories), which fail CI's
`cargo audit`. The build script uses zip only to unpack the iOS
`.xcframework.zip` archive; the zip 9 API it calls (`ZipArchive::new`,
`extract`) is unchanged, so behaviour on Windows and Linux is identical.

sherpa-onnx 2.x already uses zip 9, but it drops espeak-ng: Groovy's default
Piper voice would get no phonemes and Kokoro would skip out-of-lexicon words.

## When to remove

Drop this copy (and the `[patch.crates-io]` entry) when upstream
`sherpa-onnx-sys` moves to zip >= 9 on a release Groovy can use, or when
Groovy moves to sherpa-onnx 2.x with a permissively licensed phonemizer.
