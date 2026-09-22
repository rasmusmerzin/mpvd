#!/usr/bin/env python3
import json
import os
import sys

WIDTHS = {
    " ": 3.2, "!": 4.4, '"': 5.0, "#": 8.3, "$": 7.7, "%": 11.2, "&": 9.2,
    "'": 2.8, "(": 4.7, ")": 4.7, "*": 6.2, "+": 7.6, ",": 3.8, "-": 5.4,
    ".": 4.1, "/": 5.3, "0": 7.0, "1": 7.0, "2": 7.0, "3": 7.0, "4": 7.0,
    "5": 7.0, "6": 7.0, "7": 7.0, "8": 7.0, "9": 7.0, ":": 4.4, ";": 4.4,
    "<": 7.7, "=": 7.7, ">": 7.7, "?": 7.2, "@": 12.4,
    "A": 8.2, "B": 8.1, "C": 8.4, "D": 9.1, "E": 7.7, "F": 6.9, "G": 9.3,
    "H": 8.7, "I": 4.2, "J": 5.1, "K": 8.2, "L": 6.6, "M": 9.9, "N": 8.9,
    "O": 9.4, "P": 7.4, "Q": 9.4, "R": 8.2, "S": 7.7, "T": 6.9, "U": 8.4,
    "V": 8.2, "W": 11.9, "X": 8.2, "Y": 8.2, "Z": 7.7,
    "a": 7.2, "b": 7.2, "c": 6.1, "d": 7.2, "e": 6.7, "f": 3.9, "g": 7.2,
    "h": 7.2, "i": 3.6, "j": 3.6, "k": 6.6, "l": 3.6, "m": 11.0, "n": 7.2,
    "o": 7.0, "p": 7.2, "q": 7.2, "r": 4.7, "s": 6.0, "t": 4.4, "u": 7.2,
    "v": 6.4, "w": 9.4, "x": 6.5, "y": 6.4, "z": 6.1,
    "[": 4.7, "\\": 5.3, "]": 4.7, "^": 8.0, "_": 6.7, "`": 5.3,
    "{": 5.3, "|": 4.2, "}": 5.3, "~": 8.0,
}

COLORS = [
    (50, "#e05d44"),
    (60, "#fe7d37"),
    (70, "#dfb317"),
    (80, "#a4a61d"),
    (90, "#97ca00"),
    (100, "#4c1"),
]


def text_width(s):
    return sum(WIDTHS.get(c, 6.5) for c in s)


def load_percent(path):
    with open(path) as f:
        data = json.load(f)
    lines = data["data"][0]["totals"]["lines"]
    return lines["covered"] / lines["count"] * 100.0


def badge(pct):
    value = f"{pct:.1f}%"
    color = next((c for t, c in COLORS if pct < t), "#4c1")
    label = "coverage"
    pad = 5
    label_w = pad + text_width(label) + pad
    value_w = pad + text_width(value) + pad
    total = label_w + value_w
    lx = label_w / 2
    vx = label_w + value_w / 2
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="{total}" height="20" role="img" aria-label="{label}: {value}">
  <title>{label}: {value}</title>
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbbbbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="r"><rect width="{total}" height="20" rx="3" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="{label_w}" height="20" fill="#555"/>
    <rect x="{label_w}" width="{value_w}" height="20" fill="{color}"/>
    <rect width="{total}" height="20" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="11">
    <text aria-hidden="true" x="{lx + 0.5}" y="15" fill="#010101" fill-opacity=".3">{label}</text>
    <text x="{lx}" y="14">{label}</text>
    <text aria-hidden="true" x="{vx + 0.5}" y="15" fill="#010101" fill-opacity=".3">{value}</text>
    <text x="{vx}" y="14">{value}</text>
  </g>
</svg>
"""


def main():
    if len(sys.argv) != 3:
        sys.exit(f"usage: {sys.argv[0]} <coverage.json> <out.svg>")
    out = badge(load_percent(sys.argv[1]))
    os.makedirs(os.path.dirname(sys.argv[2]) or ".", exist_ok=True)
    with open(sys.argv[2], "w") as f:
        f.write(out)


if __name__ == "__main__":
    main()