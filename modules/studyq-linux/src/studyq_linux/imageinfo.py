"""Pixel size of a PNG or JPEG from its header (pure Python, no decoding)."""

import struct

_SOF = frozenset(range(0xC0, 0xD0)) - {0xC4, 0xC8, 0xCC}


def image_dimensions(data):
    """Return (width, height) in pixels, or None when the bytes are neither PNG nor JPEG."""
    if data[:8] == b"\x89PNG\r\n\x1a\n" and data[12:16] == b"IHDR" and len(data) >= 24:
        width, height = struct.unpack(">II", data[16:24])
        return (width, height) if width and height else None
    if data[:2] != b"\xff\xd8":
        return None
    pos, size = 2, len(data)
    while pos + 4 <= size:
        if data[pos] != 0xFF:
            return None
        marker = data[pos + 1]
        if marker == 0xFF:
            pos += 1
            continue
        if marker in _SOF:
            if pos + 9 > size:
                return None
            height, width = struct.unpack(">HH", data[pos + 5 : pos + 9])
            return (width, height) if width and height else None
        if marker == 0xD8 or 0xD0 <= marker <= 0xD7 or marker == 0x01:
            pos += 2
            continue
        pos += 2 + struct.unpack(">H", data[pos + 2 : pos + 4])[0]
    return None
