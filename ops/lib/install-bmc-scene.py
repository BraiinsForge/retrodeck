#!/usr/bin/env python3
"""Register the Retro Deck fullscreen scene in an existing BMC configuration.

Idempotent: exits successfully without writing when any scene already
contains the Retro Deck widget. Backs the original file up once next to it.

Runs on the payload's minimal MicroPython (no uuid, os.path, os.open,
os.replace, or lstat), so it sticks to builtin open, os.stat, os.rename,
and /dev/urandom, and stays valid CPython for host-side testing.
"""

import json
import os
import sys

WIDGET_UID = "73219c9d-f1ef-41dc-960c-d0711e42a6ac"
MAXIMUM_BYTES = 4 * 1024 * 1024
REGULAR_FILE = 0x8000


def error(message):
    sys.stderr.write(message + "\n")


def stat_or_none(path):
    try:
        return os.stat(path)
    except OSError:
        return None


def random_uuid():
    with open("/dev/urandom", "rb") as source:
        raw = bytearray(source.read(16))
    raw[6] = (raw[6] & 0x0F) | 0x40
    raw[8] = (raw[8] & 0x3F) | 0x80
    digits = "".join("%02x" % byte for byte in raw)
    return "%s-%s-%s-%s-%s" % (
        digits[:8], digits[8:12], digits[12:16], digits[16:20], digits[20:])


def atomic_write(path, text, permissions):
    temporary = path + ".retro-deck.tmp"
    with open(temporary, "w") as output:
        output.write(text)
    if hasattr(os, "chmod"):
        os.chmod(temporary, permissions)
    os.rename(temporary, path)


def main():
    if len(sys.argv) != 2:
        error("usage: install-bmc-scene.py BMC_CONFIG.JSON")
        return 2
    path = sys.argv[1]
    info = stat_or_none(path)
    if (info is None or (info[0] & 0xF000) != REGULAR_FILE
            or not 0 < info[6] <= MAXIMUM_BYTES):
        error("BMC configuration is not a bounded regular file")
        return 1
    with open(path) as source:
        contents = source.read()
    config = json.loads(contents)
    scenes = config.get("scenes")
    if scenes is None:
        error("BMC configuration has no scene list")
        return 1
    for scene in scenes:
        for widget in scene.get("widgets", []):
            if widget.get("widget_type_id") == WIDGET_UID:
                return 0
    scenes.append(
        {
            "id": random_uuid(),
            "enabled": True,
            "kind": "fullscreen",
            "widgets": [
                {
                    "id": random_uuid(),
                    "row": 0,
                    "col": 0,
                    "placement": "fullscreen",
                    "widget_type_id": WIDGET_UID,
                    "viewport_shape": "rectangular",
                    "params": {},
                }
            ],
        }
    )
    permissions = info[0] & 0o777
    backup = path + ".retro-deck.bak"
    backup_info = stat_or_none(backup)
    if backup_info is None:
        atomic_write(backup, contents, permissions)
    elif (backup_info[0] & 0xF000) != REGULAR_FILE:
        error("BMC configuration backup is not a regular file")
        return 1
    atomic_write(path, json.dumps(config), permissions)
    return 0


if __name__ == "__main__":
    sys.exit(main())
