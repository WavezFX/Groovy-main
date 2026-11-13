#!/usr/bin/env python3
"""Turns the round 5 design boards (docs/design/wardrobe-v2/*.dc.html) into
Canvas2D drawing data for src/mochi/art/*.ts.

Every card on a board is one SVG of Groovy (body rect x=14 y=30 w=72 h=62,
almond eyes at (38,62) and (62,62)) with an item or a growth stage drawn on
him. Each card becomes:
  behind  shapes drawn before the body (fire round him, a cap's back);
  body    shapes on the body, under the eyes (water, droplets); clipped to
          the body when the board clips them;
  front   shapes after the eyes (the plant on top, a hat, glasses, a scarf);
  iris / core  the eye colours, when the card changes them.
Circles, ellipses, rects and lines become SVG path data, so the page draws
everything with Path2D. No image file is involved.

  python3 scripts/gen-art.py            (from windows/)
"""

import json
import re
import sys
from html.parser import HTMLParser
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BOARDS = ROOT.parent / "docs" / "design" / "wardrobe-v2"
OUT = ROOT / "src" / "mochi" / "art"

STYLE = ["fill", "stroke", "stroke-width", "opacity", "fill-opacity", "stroke-linecap", "stroke-dasharray", "transform", "clip-path"]
KINDS = {"path", "circle", "ellipse", "rect", "polygon", "polyline", "line"}


class Board(HTMLParser):
    def __init__(self):
        super().__init__()
        self.cards = []
        self.cur = None
        self.stack = []
        self.clip = None
        self.after = None

    def handle_starttag(self, tag, a):
        a = dict(a)
        if tag == "svg":
            self.cur = {"shapes": [], "label": "", "tag": ""}
            self.stack = [{}]
            return
        if self.cur is None:
            return
        if tag in ("clippath", "defs"):
            self.clip = tag == "clippath"
            self.stack.append({})
            return
        if tag == "g":
            inh = dict(self.stack[-1])
            for k in STYLE:
                if k in a:
                    inh[k] = (inh.get(k, "") + " " + a[k]).strip() if k == "transform" else a[k]
            self.stack.append(inh)
            return
        if tag in KINDS and not self.clip:
            sh = {"k": tag, **{k: v for k, v in self.stack[-1].items()}}
            for k, v in a.items():
                sh[k] = (sh[k] + " " + v) if k == "transform" and "transform" in sh else v
            self.cur["shapes"].append(sh)

    def handle_endtag(self, tag):
        if self.cur is None:
            return
        if tag == "svg":
            self.cards.append(self.cur)
            self.after = self.cur
            self.cur = None
        elif tag in ("clippath", "defs"):
            self.clip = False
            self.stack.pop()
        elif tag == "g":
            self.stack.pop()

    def handle_data(self, d):
        if self.cur is None and self.after is not None and d.strip():
            if not self.after["label"]:
                self.after["label"] = d.strip()
            elif not self.after["tag"]:
                self.after["tag"] = d.strip()


def num(v):
    return float(v)


def fmt(x):
    s = f"{x:.2f}".rstrip("0").rstrip(".")
    return "0" if s == "-0" else s


def to_path(s):
    k = s["k"]
    if k == "path":
        return s["d"]
    if k == "circle":
        cx, cy, r = num(s["cx"]), num(s["cy"]), num(s["r"])
        return f"M{fmt(cx - r)} {fmt(cy)}A{fmt(r)} {fmt(r)} 0 1 0 {fmt(cx + r)} {fmt(cy)}A{fmt(r)} {fmt(r)} 0 1 0 {fmt(cx - r)} {fmt(cy)}Z"
    if k == "ellipse":
        cx, cy, rx, ry = num(s["cx"]), num(s["cy"]), num(s["rx"]), num(s["ry"])
        return f"M{fmt(cx - rx)} {fmt(cy)}A{fmt(rx)} {fmt(ry)} 0 1 0 {fmt(cx + rx)} {fmt(cy)}A{fmt(rx)} {fmt(ry)} 0 1 0 {fmt(cx - rx)} {fmt(cy)}Z"
    if k == "rect":
        x, y, w, h = num(s["x"]), num(s["y"]), num(s["width"]), num(s["height"])
        r = min(num(s.get("rx", s.get("ry", "0"))), w / 2, h / 2)
        if r <= 0:
            return f"M{fmt(x)} {fmt(y)}H{fmt(x + w)}V{fmt(y + h)}H{fmt(x)}Z"
        return (
            f"M{fmt(x + r)} {fmt(y)}H{fmt(x + w - r)}A{fmt(r)} {fmt(r)} 0 0 1 {fmt(x + w)} {fmt(y + r)}"
            f"V{fmt(y + h - r)}A{fmt(r)} {fmt(r)} 0 0 1 {fmt(x + w - r)} {fmt(y + h)}H{fmt(x + r)}"
            f"A{fmt(r)} {fmt(r)} 0 0 1 {fmt(x)} {fmt(y + h - r)}V{fmt(y + r)}A{fmt(r)} {fmt(r)} 0 0 1 {fmt(x + r)} {fmt(y)}Z"
        )
    if k == "line":
        return f"M{s['x1']} {s['y1']}L{s['x2']} {s['y2']}"
    if k in ("polygon", "polyline"):
        pts = re.findall(r"-?[\d.]+", s["points"])
        pairs = [f"{pts[i]} {pts[i + 1]}" for i in range(0, len(pts) - 1, 2)]
        return "M" + "L".join(pairs) + ("Z" if k == "polygon" else "")
    raise ValueError(k)


def is_body(s):
    return s["k"] == "rect" and s.get("x") == "14" and s.get("y") == "30" and s.get("width") == "72" and s.get("height") == "62"


def is_shadow(s):
    return s["k"] == "ellipse" and s.get("cx") == "50" and s.get("cy") == "88" and s.get("rx") == "30"


def is_eye(s):
    return s["k"] == "ellipse" and s.get("cy") == "62" and s.get("cx") in ("38", "62") and s.get("rx") in ("7", "2.4")


def shape(s):
    out = {"d": to_path(s)}
    fill = s.get("fill")
    if fill and fill != "none":
        out["f"] = fill
    stroke = s.get("stroke")
    if stroke and stroke != "none":
        out["s"] = stroke
        out["w"] = num(s.get("stroke-width", "1"))
        if s.get("stroke-linecap") == "round":
            out["cap"] = 1
        if s.get("stroke-dasharray"):
            out["dash"] = [num(v) for v in re.findall(r"[\d.]+", s["stroke-dasharray"])]
    if "f" not in out and "s" not in out:
        out["f"] = "#000000"
    o = num(s.get("opacity", "1")) * num(s.get("fill-opacity", "1"))
    if o < 0.999:
        out["o"] = round(o, 3)
    if s.get("transform"):
        out["t"] = s["transform"]
    if s.get("clip-path"):
        out["clip"] = 1
    return out


def card(c):
    shapes = c["shapes"]
    b = next((i for i, s in enumerate(shapes) if is_body(s)), None)
    behind, body, front = [], [], []
    iris = core = None
    eyes_seen = 0
    for i, s in enumerate(shapes):
        if b is not None and i < b:
            behind.append(shape(s))
        elif i == b or is_shadow(s):
            continue
        elif is_eye(s):
            eyes_seen += 1
            if s["rx"] == "7":
                iris = s.get("fill")
            else:
                core = s.get("fill")
        elif eyes_seen == 0:
            body.append(shape(s))
        else:
            front.append(shape(s))
    out = {"label": c["label"], "tag": c["tag"], "behind": behind, "body": body, "front": front}
    if iris:
        out["iris"] = iris
    if core:
        out["core"] = core
    return out


def board(name):
    p = Board()
    p.feed((BOARDS / f"{name}.dc.html").read_text())
    return [card(c) for c in p.cards]


def ts_value(v):
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)


HEADER = "// Generated by scripts/gen-art.py from docs/design/wardrobe-v2/{board}.dc.html.\n// Do not edit by hand: change the board and run the script again.\n\nimport type {{ Art }} from \"./draw\";\n\n"


def write(file, board_name, entries):
    lines = [HEADER.format(board=board_name)]
    for const, value in entries:
        lines.append(f"export const {const}: {value[0]} = {ts_value(value[1])};\n\n")
    (OUT / file).write_text("".join(lines).rstrip() + "\n")


def main():
    which = set(sys.argv[1:]) or {"growth", "stage6", "wardrobe", "seasonal"}
    OUT.mkdir(parents=True, exist_ok=True)
    if "growth" in which:
        cards = board("Main")
        looks = {"night": cards[0:6], "ember": cards[6:12], "frost": cards[12:18]}
        write("growth.ts", "Main", [("GROWTH_ART", ("Record<\"night\" | \"ember\" | \"frost\", Art[]>", {k: [strip(c) for c in v] for k, v in looks.items()}))])
    if "stage6" in which:
        cards = board("Stage6")
        write("stage6.ts", "Stage6", [("STAGE6_ART", ("Record<\"night\" | \"ember\" | \"frost\", Art>", {k: strip(c) for k, c in zip(["night", "ember", "frost"], cards)}))])
    if "wardrobe" in which:
        cards = board("Wardrobe")[:28]
        write("wardrobe.ts", "Wardrobe", [("WARDROBE_ART", ("Record<string, Art>", {slug(c["label"]): strip(c) for c in cards}))])
    if "seasonal" in which:
        cards = board("Seasonal")
        write("seasonal.ts", "Seasonal", [("SEASONAL_ART", ("Record<string, Art>", {slug(c["label"]): strip(c) for c in cards}))])


def strip(c):
    return {k: v for k, v in c.items() if k not in ("label", "tag") and v not in ([], None)}


def slug(label):
    words = re.sub(r"[^a-zA-Z0-9 ]", " ", label.replace("&", " ")).split()
    return words[0].lower() + "".join(w.capitalize() for w in words[1:])


if __name__ == "__main__":
    main()
