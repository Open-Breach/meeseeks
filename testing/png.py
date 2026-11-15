#!/usr/bin/env python3
"""Says what is in a PNG file: its size in pixels and the pixels asked for.
Usage: png.py file.png [x,y ...]

It reads the file itself, so it needs no imaging library. It knows the PNG
files GTK writes: 8 or 16 bits a channel, with or without alpha, not
interlaced.
"""
import struct
import sys
import zlib


def load(path):
    data = open(path, "rb").read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG file"
    pos, packed, header = 8, b"", None
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            packed += body
        pos += 12 + length
    width, height, depth, colour, _, _, interlaced = header
    channels = {0: 1, 2: 3, 4: 2, 6: 4}[colour]
    assert depth in (8, 16) and not interlaced, header
    step = channels * depth // 8
    stride = width * step
    raw = zlib.decompress(packed)
    rows, before = [], bytes(stride)
    for y in range(height):
        kind = raw[y * (stride + 1)]
        row = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            left = row[i - step] if i >= step else 0
            up = before[i]
            corner = before[i - step] if i >= step else 0
            if kind == 1:
                row[i] = (row[i] + left) & 255
            elif kind == 2:
                row[i] = (row[i] + up) & 255
            elif kind == 3:
                row[i] = (row[i] + (left + up) // 2) & 255
            elif kind == 4:
                p = left + up - corner
                nearest = min((abs(p - left), 0, left), (abs(p - up), 1, up), (abs(p - corner), 2, corner))[2]
                row[i] = (row[i] + nearest) & 255
        rows.append(row)
        before = row
    return width, height, depth, channels, rows


def main():
    width, height, depth, channels, rows = load(sys.argv[1])
    kind = {1: "grey", 2: "grey with alpha", 3: "RGB", 4: "RGBA"}[channels]
    print(f"{width}x{height}, {depth} bits, {kind}")
    size = depth // 8
    for arg in sys.argv[2:]:
        x, y = map(int, arg.split(","))
        if not (0 <= x < width and 0 <= y < height):
            print(f"{x},{y}: outside")
            continue
        at = x * channels * size
        # The high byte of each channel says enough.
        print(f"{x},{y}: {tuple(rows[y][at + c * size] for c in range(channels))}")


if __name__ == "__main__":
    main()
