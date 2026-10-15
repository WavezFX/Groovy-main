#!/usr/bin/env python3
"""Synthesises Groovy's 29 sounds into windows/sounds/ — original sounds made
in code, so nothing here comes from another app. Soft, warm and short.

    python3 scripts/gen-sounds.py
"""

import os
import wave

import numpy as np

SR = 48000
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "sounds")


def t_of(dur):
    return np.arange(int(SR * dur)) / SR


def env(n, attack=0.005, release=0.08, curve=3.0):
    """Quick attack, exponential-ish release over the whole length."""
    t = np.arange(n) / SR
    a = np.clip(t / max(attack, 1e-4), 0, 1)
    dur = n / SR
    r = np.clip((dur - t) / max(release, 1e-4), 0, 1)
    decay = np.exp(-curve * t / max(dur, 1e-4))
    return a * r * decay


def bell(freq, dur, amp=0.5, bright=0.35, curve=4.0):
    """A soft bell: sine plus a quiet inharmonic partial."""
    t = t_of(dur)
    s = np.sin(2 * np.pi * freq * t) + bright * np.sin(2 * np.pi * freq * 2.76 * t) * np.exp(-8 * t)
    s += 0.15 * np.sin(2 * np.pi * freq * 2 * t)
    return amp * s * env(len(t), 0.004, 0.05, curve)


def tone(freq, dur, amp=0.5, shape="sine", curve=3.0, vib=0.0, vib_rate=6.0):
    t = t_of(dur)
    f = freq * (1 + vib * np.sin(2 * np.pi * vib_rate * t))
    ph = 2 * np.pi * np.cumsum(f) / SR
    if shape == "tri":
        s = 2 / np.pi * np.arcsin(np.sin(ph))
    elif shape == "soft-square":
        s = np.tanh(3 * np.sin(ph)) * 0.7
    else:
        s = np.sin(ph)
    return amp * s * env(len(t), 0.006, 0.05, curve)


def glide(f0, f1, dur, amp=0.5, shape="sine", curve=3.0, vib=0.0):
    t = t_of(dur)
    f = f0 * (f1 / f0) ** (t / dur)
    f = f * (1 + vib * np.sin(2 * np.pi * 7 * t))
    ph = 2 * np.pi * np.cumsum(f) / SR
    s = np.sin(ph) if shape == "sine" else 2 / np.pi * np.arcsin(np.sin(ph))
    return amp * s * env(len(t), 0.004, 0.04, curve)


def noise(dur, amp=0.3, lp=0.15, curve=12.0):
    n = int(SR * dur)
    x = np.random.default_rng(7).standard_normal(n)
    y = np.zeros(n)
    acc = 0.0
    for i in range(n):  # one-pole low-pass
        acc += lp * (x[i] - acc)
        y[i] = acc
    y /= np.max(np.abs(y)) + 1e-9
    return amp * y * env(n, 0.001, 0.02, curve)


def mix(*parts):
    """Adds sounds of different lengths together."""
    return seq(*[(0, p) for p in parts])


def seq(*parts):
    """Parts are (start_seconds, samples); mixed into one buffer."""
    end = max(int(start * SR) + len(p) for start, p in parts)
    out = np.zeros(end + int(0.02 * SR))
    for start, p in parts:
        i = int(start * SR)
        out[i:i + len(p)] += p
    return out


def note(name):
    names = {"C": -9, "D": -7, "E": -5, "F": -4, "G": -2, "A": 0, "B": 2}
    semis = names[name[0]] + (1 if "#" in name else 0)
    octave = int(name[-1])
    return 440.0 * 2 ** ((semis + 12 * (octave - 4)) / 12)


def save(name, s, gain=0.8):
    s = np.asarray(s, dtype=float)
    peak = np.max(np.abs(s)) + 1e-9
    s = s / peak * gain
    # A touch of stereo width: the right channel a hair later.
    d = int(0.0006 * SR)
    left = s
    right = np.concatenate([np.zeros(d), s[:-d]]) if d else s
    data = np.stack([left, right], axis=1)
    pcm = (np.clip(data, -1, 1) * 32767).astype("<i2")
    with wave.open(os.path.join(OUT, f"{name}.wav"), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())


def main():
    os.makedirs(OUT, exist_ok=True)
    N = note
    sounds = {
        "pop": glide(900, 380, 0.09, curve=5),
        "blip": tone(1320, 0.05, curve=6),
        "tick": mix(tone(2200, 0.018, curve=8), noise(0.018, 0.2, 0.5)),
        "hover": tone(760, 0.06, amp=0.4, curve=5),
        "open": seq((0, bell(N("C5"), 0.18)), (0.07, bell(N("G5"), 0.25))),
        "close": seq((0, bell(N("G5"), 0.16)), (0.07, bell(N("C5"), 0.22))),
        "peek": glide(500, 1100, 0.14, amp=0.45, curve=3),
        "greet": seq((0, bell(N("C5"), 0.25)), (0.09, bell(N("E5"), 0.25)), (0.18, bell(N("G5"), 0.45))),
        "greeting": seq(
            (0, bell(N("G4"), 0.25)), (0.1, bell(N("C5"), 0.25)),
            (0.2, bell(N("E5"), 0.25)), (0.3, bell(N("G5"), 0.6, bright=0.5)),
        ),
        "finish": seq(
            (0, bell(N("C5"), 0.3)), (0.08, bell(N("E5"), 0.3)),
            (0.16, bell(N("G5"), 0.3)), (0.26, bell(N("C6"), 0.8, bright=0.5, curve=3)),
        ),
        "approval": seq((0, bell(N("E5"), 0.3, bright=0.5)), (0.16, bell(N("B5"), 0.3, bright=0.5)),
                        (0.42, bell(N("E5"), 0.25)), (0.56, bell(N("B5"), 0.3))),
        "approve": seq((0, tone(N("G5"), 0.08, shape="tri")), (0.07, bell(N("C6"), 0.3))),
        "question": seq((0, bell(N("C5"), 0.2)), (0.12, glide(N("C5"), N("F5"), 0.28, amp=0.45))),
        "error": seq((0, tone(N("E4"), 0.18, shape="soft-square", amp=0.35)),
                     (0.16, tone(N("C4"), 0.32, shape="soft-square", amp=0.35))),
        "rate": seq((0, bell(N("G4"), 0.25)), (0.18, bell(N("E4"), 0.25)), (0.36, bell(N("C4"), 0.5))),
        "attach": seq((0, glide(300, 1400, 0.16, amp=0.25, shape="tri")), (0.16, tone(1800, 0.03, curve=8))),
        "send": mix(glide(400, 1600, 0.22, amp=0.4, curve=2), noise(0.22, 0.12, 0.05, 4)),
        "gulp": seq((0, glide(700, 220, 0.14, curve=3)), (0.12, tone(110, 0.12, amp=0.6, curve=6))),
        "search": seq((0, bell(1046, 0.6, amp=0.4, bright=0.2, curve=2.5)), (0.25, bell(1046, 0.5, amp=0.15, curve=3))),
        "think": seq((0, tone(660, 0.07, amp=0.35)), (0.12, tone(784, 0.07, amp=0.35)), (0.24, tone(880, 0.09, amp=0.3))),
        "work": seq(*[(i * 0.07, mix(tone(1500 + 120 * (i % 2), 0.025, curve=8), noise(0.02, 0.15, 0.4))) for i in range(4)]),
        "slap": mix(noise(0.09, 0.8, 0.35, 18), tone(180, 0.08, amp=0.4, curve=10)),
        "dizzy": glide(900, 300, 0.7, amp=0.4, curve=1.5, vib=0.08),
        "annoyed": seq((0, tone(N("D4"), 0.14, shape="tri", amp=0.45)), (0.13, tone(N("C#4"), 0.25, shape="tri", amp=0.45))),
        "love": seq((0, tone(N("E5"), 0.3, vib=0.01)), (0.18, tone(N("A5"), 0.5, vib=0.012, curve=2.5))),
        "proud": seq((0, bell(N("G4"), 0.15)), (0.1, bell(N("C5"), 0.15)), (0.2, bell(N("E5"), 0.15)),
                     (0.3, bell(N("G5"), 0.15)), (0.4, bell(N("C6"), 0.5, bright=0.5))),
        "wink": glide(1100, 1700, 0.08, amp=0.4, curve=4),
        "sleep": seq((0, tone(N("E4"), 0.5, amp=0.35, curve=2)), (0.35, tone(N("C4"), 0.8, amp=0.3, curve=2))),
        "yawn": glide(520, 240, 0.9, amp=0.35, shape="tri", curve=1.2, vib=0.015),
    }
    for name, s in sounds.items():
        save(name, s)
    print(f"{len(sounds)} sounds written to {os.path.normpath(OUT)}")


if __name__ == "__main__":
    main()
