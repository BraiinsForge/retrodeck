#!/usr/bin/env python3
"""Register the Retro Deck fullscreen scene in an existing BMC configuration.

Idempotent: exits successfully without writing when any scene already
contains the Retro Deck widget. Backs the original file up once next to it.
"""

import json
import os
import sys
import uuid

WIDGET_UID = "73219c9d-f1ef-41dc-960c-d0711e42a6ac"
MAXIMUM_BYTES = 4 * 1024 * 1024


def atomic_write(path, text, mode):
    temporary = path + ".retro-deck.tmp"
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, mode)
    with os.fdopen(descriptor, "w") as output:
        output.write(text)
    os.replace(temporary, path)


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: install-bmc-scene.py BMC_CONFIG.JSON", file=sys.stderr)
        return 2
    path = sys.argv[1]
    info = os.lstat(path)
    if not os.path.isfile(path) or not 0 < info.st_size <= MAXIMUM_BYTES:
        print("BMC configuration is not a bounded regular file", file=sys.stderr)
        return 1
    with open(path) as source:
        contents = source.read()
    config = json.loads(contents)
    scenes = config.get("scenes")
    if scenes is None:
        print("BMC configuration has no scene list", file=sys.stderr)
        return 1
    for scene in scenes:
        for widget in scene.get("widgets", []):
            if widget.get("widget_type_id") == WIDGET_UID:
                return 0
    scenes.append(
        {
            "id": str(uuid.uuid4()),
            "enabled": True,
            "kind": "fullscreen",
            "widgets": [
                {
                    "id": str(uuid.uuid4()),
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
    permissions = info.st_mode & 0o777
    backup = path + ".retro-deck.bak"
    if not os.path.exists(backup):
        atomic_write(backup, contents, permissions)
    elif not os.path.isfile(backup):
        print("BMC configuration backup is not a regular file", file=sys.stderr)
        return 1
    atomic_write(path, json.dumps(config, indent=2) + "\n", permissions)
    return 0


if __name__ == "__main__":
    sys.exit(main())
