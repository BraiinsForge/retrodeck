#!/usr/bin/env python3

"""Render a DOOM menu cover from a WAD's own title graphic.

The DECK carousel draws `covers/<id>.png` for each entry. Shipping a DOOM
logo in this repository would mean redistributing id Software artwork, so the
cover is produced at deployment time from the owner's own IWAD instead. Only
the Python standard library is used: the Deck payload has no image tooling and
the build machine should not need one either.

Usage: extract-doom-cover.py IWAD OUTPUT.png
"""

import struct
import sys
import zlib

# Graphics to try, best first. TITLEPIC is the full title screen and makes the
# most recognisable cover; INTERPIC and M_DOOM are fallbacks for IWADs that
# name things differently.
CANDIDATE_LUMPS = ("TITLEPIC", "INTERPIC", "M_DOOM")

# DOOM's 320x200 pixels are not square; they were displayed as 4:3. Scaling
# the height by 6/5 restores the proportions the art was drawn for.
ASPECT_NUMERATOR = 6
ASPECT_DENOMINATOR = 5

MAXIMUM_LUMPS = 65536
MAXIMUM_DIMENSION = 2048


def fail(message):
    sys.stderr.write("extract-doom-cover: %s\n" % message)
    raise SystemExit(1)


def read_directory(data):
    if len(data) < 12 or data[0:4] not in (b"IWAD", b"PWAD"):
        fail("not a DOOM WAD")
    count, offset = struct.unpack_from("<ii", data, 4)
    if not 0 < count <= MAXIMUM_LUMPS:
        fail("implausible lump count %d" % count)
    if offset < 0 or offset + count * 16 > len(data):
        fail("lump directory lies outside the file")

    directory = {}
    for index in range(count):
        position, size, raw_name = struct.unpack_from(
            "<ii8s", data, offset + index * 16
        )
        name = raw_name.split(b"\0")[0].decode("ascii", "replace")
        if position < 0 or size < 0 or position + size > len(data):
            continue
        # First definition wins, matching how DOOM resolves duplicates for
        # these graphics.
        directory.setdefault(name, (position, size))
    return directory


def read_palette(data, directory):
    if "PLAYPAL" not in directory:
        fail("WAD has no PLAYPAL palette")
    position, size = directory["PLAYPAL"]
    if size < 768:
        fail("PLAYPAL is too short")
    raw = data[position:position + 768]
    return [tuple(raw[i * 3:i * 3 + 3]) for i in range(256)]


def decode_patch(patch, palette):
    """Decode DOOM's column-major picture format into RGB rows."""
    if len(patch) < 8:
        fail("graphic is too short")
    width, height, _left, _top = struct.unpack_from("<hhhh", patch, 0)
    if not 0 < width <= MAXIMUM_DIMENSION or not 0 < height <= MAXIMUM_DIMENSION:
        fail("graphic has implausible dimensions %dx%d" % (width, height))
    if 8 + width * 4 > len(patch):
        fail("graphic column table is truncated")

    columns = struct.unpack_from("<%dI" % width, patch, 8)
    # Uncovered pixels are transparent in DOOM; on a cover they read as black.
    rows = [bytearray(width * 3) for _ in range(height)]

    for x, column_offset in enumerate(columns):
        cursor = column_offset
        if cursor >= len(patch):
            continue
        while True:
            if cursor >= len(patch):
                break
            top_delta = patch[cursor]
            if top_delta == 0xFF:
                break
            if cursor + 2 >= len(patch):
                break
            length = patch[cursor + 1]
            # One padding byte precedes and follows every post's pixels.
            start = cursor + 3
            end = start + length
            if end > len(patch):
                break
            for offset in range(length):
                y = top_delta + offset
                if 0 <= y < height:
                    red, green, blue = palette[patch[start + offset]]
                    rows[y][x * 3] = red
                    rows[y][x * 3 + 1] = green
                    rows[y][x * 3 + 2] = blue
            cursor = end + 1
    return width, height, rows


def correct_aspect(width, height, rows):
    target = height * ASPECT_NUMERATOR // ASPECT_DENOMINATOR
    if target <= height:
        return width, height, rows
    scaled = []
    for y in range(target):
        source = y * height // target
        scaled.append(rows[source])
    return width, target, scaled


def write_png(path, width, height, rows):
    def chunk(kind, payload):
        body = kind + payload
        return (
            struct.pack(">I", len(payload))
            + body
            + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)
        )

    raw = bytearray()
    for row in rows:
        raw.append(0)  # filter type 0: no prediction
        raw.extend(row)

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    with open(path, "wb") as output:
        output.write(b"\x89PNG\r\n\x1a\n")
        output.write(chunk(b"IHDR", header))
        output.write(chunk(b"IDAT", zlib.compress(bytes(raw), 9)))
        output.write(chunk(b"IEND", b""))


def main(arguments):
    if len(arguments) != 2:
        sys.stderr.write(__doc__)
        raise SystemExit(2)
    wad_path, output_path = arguments

    with open(wad_path, "rb") as handle:
        data = handle.read()

    directory = read_directory(data)
    palette = read_palette(data, directory)

    for name in CANDIDATE_LUMPS:
        if name in directory:
            position, size = directory[name]
            width, height, rows = decode_patch(
                data[position:position + size], palette
            )
            width, height, rows = correct_aspect(width, height, rows)
            write_png(output_path, width, height, rows)
            sys.stderr.write(
                "extract-doom-cover: %s %dx%d from %s\n"
                % (output_path, width, height, name)
            )
            return 0

    fail("WAD has none of %s" % ", ".join(CANDIDATE_LUMPS))


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
