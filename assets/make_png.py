#!/usr/bin/env python3
"""Generate simple valid PNG files (RGBA) without external dependencies.

Usage: python3 make_png.py width height out.png
Or run without args to create default icons.
"""
import sys
import struct
import zlib

def crc(chunk_type, data=b""):
    return zlib.crc32(chunk_type + data) & 0xffffffff

def make_png(width, height, out_path):
    # PNG signature
    sig = b"\x89PNG\r\n\x1a\n"

    # IHDR
    ihdr_data = struct.pack("!IIBBBBB", width, height, 8, 6, 0, 0, 0)
    ihdr = b"IHDR" + ihdr_data
    ihdr_chunk = struct.pack("!I", len(ihdr_data)) + ihdr + struct.pack("!I", crc(b"IHDR", ihdr_data))

    # Raw image data: RGBA
    # Each scanline starts with filter byte 0
    row = b"\x00" + b"\x00\x00\x00\x00" * width
    raw = row * height
    comp = zlib.compress(raw)
    idat = b"IDAT" + comp
    idat_chunk = struct.pack("!I", len(comp)) + idat + struct.pack("!I", crc(b"IDAT", comp))

    # IEND
    iend_chunk = struct.pack("!I", 0) + b"IEND" + struct.pack("!I", crc(b"IEND", b""))

    with open(out_path, "wb") as f:
        f.write(sig)
        f.write(ihdr_chunk)
        f.write(idat_chunk)
        f.write(iend_chunk)
    print(f"Wrote {out_path} ({width}x{height})")

def main():
    if len(sys.argv) == 4:
        w = int(sys.argv[1])
        h = int(sys.argv[2])
        out = sys.argv[3]
        make_png(w,h,out)
        return
    # default
    make_png(512, 512, "assets/icon-512.png")
    make_png(256, 256, "assets/icon-256.png")
    make_png(256, 256, "assets/icon.png")

if __name__ == '__main__':
    main()
