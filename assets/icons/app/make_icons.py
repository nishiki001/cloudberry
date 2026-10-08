#!/usr/bin/env python3
"""Builds cloudberry.ico (Windows) and cloudberry.icns (macOS) from png/cloudberry-<size>.png.
Pure standard library: both containers embed the PNG files as they are. Run at release time
(.github/workflows/build-setup.yml); the outputs are not committed."""
import os, struct

HERE = os.path.dirname(os.path.abspath(__file__))
png = lambda n: open(f"{HERE}/png/cloudberry-{n}.png", "rb").read()

# ICO: header, one directory entry per image, then the PNG payloads (Vista+ reads PNG entries)
sizes = [16, 24, 32, 48, 64, 128, 256]
offset = 6 + 16 * len(sizes)
head, body = struct.pack("<HHH", 0, 1, len(sizes)), b""
for s in sizes:
    data = png(s)
    head += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(data), offset + len(body))
    body += data
open(f"{HERE}/cloudberry.ico", "wb").write(head + body)

# ICNS: 'icns' + length, then (type, length, PNG) chunks
types = {16: b"icp4", 32: b"icp5", 64: b"icp6", 128: b"ic07", 256: b"ic08", 512: b"ic09", 1024: b"ic10"}
chunks = b"".join(t + struct.pack(">I", 8 + len(png(s))) + png(s) for s, t in types.items())
open(f"{HERE}/cloudberry.icns", "wb").write(b"icns" + struct.pack(">I", 8 + len(chunks)) + chunks)
print("wrote cloudberry.ico, cloudberry.icns")
