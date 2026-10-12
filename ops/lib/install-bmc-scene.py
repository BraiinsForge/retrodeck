#!/usr/bin/env python3
"""Register the Retro Deck fullscreen scene in an existing BMC configuration.

Accepts the stock BMC version-2 schema only. Preserves existing scenes and
configured instance UUIDs; backs the original file up once next to it.

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
    if stat_or_none(temporary) is not None:
        raise ValueError("BMC configuration temporary file already exists")
    with open(temporary, "w") as output:
        output.write(text)
    if hasattr(os, "chmod"):
        os.chmod(temporary, permissions)
    os.rename(temporary, path)


def valid_uuid(value):
    if not isinstance(value, str) or len(value) != 36:
        return False
    for index, char in enumerate(value):
        if index in (8, 13, 18, 23):
            if char != "-":
                return False
        elif char.lower() not in "0123456789abcdef":
            return False
    return True


def validate_config(config):
    if not isinstance(config, dict) or config.get("version") != 2:
        raise ValueError("Stock BMC config version 2 is required; migrate with BMC first")
    scenes = config.get("scenes")
    if not isinstance(scenes, list):
        raise ValueError("BMC configuration has no scene list")
    ids = []
    running = 0
    instances = []
    for scene in scenes:
        if not isinstance(scene, dict) or not valid_uuid(scene.get("id")):
            raise ValueError("Invalid BMC scene UUID")
        if scene["id"] in ids:
            raise ValueError("Duplicate BMC scene UUID")
        ids.append(scene["id"])
        if not isinstance(scene.get("enabled"), bool):
            raise ValueError("Invalid BMC scene enabled flag")
        widgets = scene.get("widgets")
        if not isinstance(widgets, list) or scene.get("kind") not in ("fullscreen", "combined"):
            raise ValueError("Invalid BMC scene shape")
        if scene["enabled"]:
            running += len(widgets)
        for widget in widgets:
            if not isinstance(widget, dict) or not valid_uuid(widget.get("id")):
                raise ValueError("Invalid configured widget UUID")
            if widget["id"] in ids:
                raise ValueError("Duplicate configured widget UUID")
            ids.append(widget["id"])
            if not valid_uuid(widget.get("widget_type_id")):
                raise ValueError("Invalid widget type UUID")
            if widget["widget_type_id"] == WIDGET_UID:
                if (scene["kind"] != "fullscreen" or len(widgets) != 1
                        or widget.get("placement") != "fullscreen"
                        or widget.get("row") != 0 or widget.get("col") != 0
                        or widget.get("viewport_shape", "rectangular") != "rectangular"):
                    raise ValueError("Existing Retro Deck widget is not a fullscreen rectangular scene")
                instances.append(widget["id"])
    if running > 56 or (not instances and running == 56):
        raise ValueError("BMC running-widget limit (56) would be exceeded")
    return scenes, instances


def main():
    args = sys.argv[1:]
    mode = "install"
    if args and args[0] in ("--check", "--widget-ids", "--enabled-widget-ids"):
        mode = args.pop(0)
    if len(args) != 1:
        error("usage: install-bmc-scene.py [--check|--widget-ids|--enabled-widget-ids] BMC_CONFIG.JSON")
        return 2
    path = args[0]
    info = stat_or_none(path)
    if (info is None or (info[0] & 0xF000) != REGULAR_FILE
            or not 0 < info[6] <= MAXIMUM_BYTES):
        raise ValueError("BMC configuration is not a bounded regular file")
    with open(path) as source:
        contents = source.read()
    config = json.loads(contents)
    scenes, instances = validate_config(config)
    if mode in ("--widget-ids", "--enabled-widget-ids"):
        if not instances:
            raise ValueError("Retro Deck configured widget is missing")
        for scene in scenes:
            if mode == "--widget-ids" or scene["enabled"]:
                for widget in scene["widgets"]:
                    if widget["widget_type_id"] == WIDGET_UID:
                        print(widget["id"])
        return 0
    if mode == "--check" or instances:
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
    try:
        result = main()
    except (ValueError, OSError) as problem:
        error(str(problem))
        result = 1
    sys.exit(result)
