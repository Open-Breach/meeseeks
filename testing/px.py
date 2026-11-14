#!/usr/bin/env python3
"""Reads pixels out of a binary PPM screenshot. Usage: px.py shot.ppm x,y [x,y ...]"""
import sys

def load(path):
    data = open(path, "rb").read()
    parts, pos = [], 0
    while len(parts) < 4:
        while data[pos:pos + 1].isspace():
            pos += 1
        end = pos
        while not data[end:end + 1].isspace():
            end += 1
        parts.append(data[pos:end]); pos = end
    assert parts[0] == b"P6" and parts[3] == b"255", parts
    return int(parts[1]), int(parts[2]), data[pos + 1:]

w, h, px = load(sys.argv[1])
for arg in sys.argv[2:]:
    x, y = map(int, arg.split(","))
    if 0 <= x < w and 0 <= y < h:
        print(f"{x},{y}: {tuple(px[(y * w + x) * 3:(y * w + x) * 3 + 3])}")
    else:
        print(f"{x},{y}: outside the {w}x{h} screenshot")
