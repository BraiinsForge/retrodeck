#!/usr/bin/env python3
"""Offline stock BMC v2 configuration, profile installation, and health checks."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]
PROVISION = REPO / "ops/lib/install-bmc-scene.py"
INSTALL = REPO / "ops/lib/install-stock-widget.sh"
UID = "73219c9d-f1ef-41dc-960c-d0711e42a6ac"
STORE = "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-bmc-widget-retro-deck-1.0.0"


def uuid(number):
    return "00000000-0000-4000-8000-%012d" % number


def existing_config():
    return {"version": 2, "brightness_pct": 47, "localization": {"timezone": "Europe/Prague"},
            "scenes": [{"id": uuid(1), "enabled": True, "kind": "combined",
                        "widgets": [{"id": uuid(2), "row": 0, "col": 0,
                                     "placement": {"slot_span": {"columns": 2, "rows": 1}},
                                     "widget_type_id": uuid(3), "params": {"mode": "flat"},
                                     "credential_bindings": {"account": uuid(4)}}]}]}


class StockBmcInstall(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.config = self.root / "etc/bmc/config.json"
        self.config.parent.mkdir(parents=True)
        self.config.write_text(json.dumps(existing_config()))
        self.config.chmod(0o600)
        self.stage = self.root / "stage"
        (self.stage / "nes-deck/langs").mkdir(parents=True)
        (self.stage / "nes-deck/langs/python").symlink_to(sys.executable)
        (self.stage / "install-bmc-scene.py").write_bytes(PROVISION.read_bytes())
        (self.stage / "widget-store-path").write_text(STORE + "\n")
        (self.stage / "widget.nar").write_bytes(b"mock-package-export")
        self.profile = self.root / "run/current-profile"
        (self.profile / "bin").mkdir(parents=True)
        (self.profile / "lib/bmc-widgets").mkdir(parents=True)
        (self.profile / "manifest").write_text('{"packages":{"owner-app":{"version":"2.0"}}}')
        self.script("etc/init.d/bmc-compositor", "exit 0")
        self.script("run/current-profile/bin/nix-store", 'cat >/dev/null; printf "import\\n" >>"$TEST_ROOT/calls"')
        self.script("run/current-profile/bin/bmc-nix-cli", '''[ "$1" != is-initialized ] || exit 0
printf '%s\\n' "$*" >>"$TEST_ROOT/calls"
mkdir -p "$TEST_ROOT/run/current-profile/lib/bmc-widgets/retro-deck"
cp -R "$TEST_ROOT"''' + STORE + '''/lib/bmc-widgets/retro-deck/. "$TEST_ROOT/run/current-profile/lib/bmc-widgets/retro-deck/"''')
        package = self.root / STORE[1:] / "lib/bmc-widgets/retro-deck"
        package.mkdir(parents=True)
        (package / "manifest.json").write_bytes((REPO / "deploy/widget/manifest.json").read_bytes())
        self.script(STORE[1:] + "/lib/bmc-widgets/retro-deck/bin/retro-deck", "exit 0")

    def script(self, path, contents):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("#!/bin/sh\n" + contents + "\n")
        target.chmod(0o755)

    def provision(self, mode=None, success=True):
        args = [sys.executable, str(PROVISION)]
        if mode:
            args.append(mode)
        result = subprocess.run(args + [str(self.config)], capture_output=True, text=True)
        self.assertEqual(result.returncode == 0, success, result.stderr)
        return result

    def install(self, mode="install", success=True):
        result = subprocess.run(["sh", str(INSTALL), mode, str(self.stage), str(self.root)],
                                env={**os.environ, "TEST_ROOT": str(self.root)}, capture_output=True, text=True)
        self.assertEqual(result.returncode == 0, success, result.stderr)
        return result

    def test_append_preserve_backup_permissions_and_idempotence(self):
        original = self.config.read_bytes()
        self.provision("--check")
        self.assertEqual(self.config.read_bytes(), original)
        self.provision()
        config = json.loads(self.config.read_text())
        self.assertEqual(config["scenes"][:-1], existing_config()["scenes"])
        self.assertEqual(config["brightness_pct"], 47)
        widget = config["scenes"][-1]["widgets"][0]
        self.assertEqual(widget["widget_type_id"], UID)
        self.assertNotEqual(widget["id"], UID)
        self.assertEqual(widget["placement"], "fullscreen")
        self.assertEqual(Path(str(self.config) + ".retro-deck.bak").read_bytes(), original)
        self.assertEqual(self.config.stat().st_mode & 0o777, 0o600)
        installed = self.config.read_bytes()
        self.provision()
        self.assertEqual(self.config.read_bytes(), installed)
        self.assertEqual(self.provision("--widget-ids").stdout.strip(), widget["id"])

    def test_unsupported_schema_or_shape_is_not_written(self):
        for version in (None, 0, 1, 3):
            config = existing_config()
            config["version"] = version
            self.config.write_text(json.dumps(config))
            before = self.config.read_bytes()
            self.provision(success=False)
            self.assertEqual(self.config.read_bytes(), before)
        self.config.write_text('{"version":2,"scenes":{}}')
        self.provision(success=False)

    def test_existing_disabled_instance_is_preserved(self):
        self.provision()
        config = json.loads(self.config.read_text())
        config["scenes"][-1]["enabled"] = False
        self.config.write_text(json.dumps(config))
        before = self.config.read_bytes()
        self.provision()
        self.assertEqual(self.config.read_bytes(), before)
        self.assertEqual(self.provision("--enabled-widget-ids").stdout, "")
        self.assertNotEqual(self.provision("--widget-ids").stdout, "")

    def test_malformed_existing_deck_is_refused(self):
        self.provision()
        config = json.loads(self.config.read_text())
        config["scenes"][-1]["widgets"][0]["row"] = 1
        self.config.write_text(json.dumps(config))
        self.provision(success=False)

    def test_profile_install_uses_stock_commands_without_replacing_manifest(self):
        original = (self.profile / "manifest").read_bytes()
        self.install("check")
        self.assertFalse((self.root / "calls").exists())
        self.install()
        self.assertEqual((self.root / "calls").read_text().splitlines(),
                         ["import", "add-packages --name retro-deck-widget --version 1.0.0 --store-path " + STORE])
        self.assertEqual((self.profile / "manifest").read_bytes(), original)
        self.assertTrue((self.profile / "lib/bmc-widgets/retro-deck/manifest.json").exists())
        self.assertEqual(json.loads(self.config.read_text())["scenes"][-1]["widgets"][0]["widget_type_id"], UID)

    def test_import_or_profile_failure_does_not_commit_scene(self):
        for failed_command in ("nix-store", "bmc-nix-cli"):
            with self.subTest(command=failed_command):
                executable = self.profile / "bin" / failed_command
                original = executable.read_text()
                self.script("run/current-profile/bin/" + failed_command,
                            '[ "$1" != is-initialized ] || exit 0\nexit 3')
                before = self.config.read_bytes()
                self.install(success=False)
                self.assertEqual(self.config.read_bytes(), before)
                self.assertFalse(Path(str(self.config) + ".retro-deck.bak").exists())
                executable.write_text(original)

    def test_missing_active_widget_does_not_commit_scene(self):
        self.script("run/current-profile/bin/bmc-nix-cli", "exit 0")
        before = self.config.read_bytes()
        self.install(success=False)
        self.assertEqual(self.config.read_bytes(), before)
        self.assertFalse(Path(str(self.config) + ".retro-deck.bak").exists())

    def test_missing_stock_prerequisites_are_refused_before_import(self):
        (self.profile / "manifest").unlink()
        self.install(success=False)
        self.assertFalse((self.root / "calls").exists())

    def test_unmounted_store_is_refused_before_import(self):
        self.script("run/current-profile/bin/bmc-nix-cli", "exit 3")
        self.install(success=False)
        self.assertFalse((self.root / "calls").exists())

    def test_invalid_store_path_is_refused_before_import(self):
        for value in ("/tmp/widget", STORE + ";evil", STORE + "\n/nix/store/another"):
            (self.stage / "widget-store-path").write_text(value)
            self.install(success=False)
            self.assertFalse((self.root / "calls").exists())

    def health(self):
        source = (REPO / "ops/check-deck.sh").read_text()
        source = source.split("# Presentation state", 1)[1].split("# End presentation check.", 1)[0]
        source = source[source.index("dashboard_expected="):]
        for path in ("/mnt/data/", "/run/current-profile/", "/etc/"):
            source = source.replace(path, str(self.root) + path)
        result = subprocess.run(["sh", "-c", 'failures=0; report() { printf "%s %s\\n" "$1" "$2"; };\n' + source + '\nexit "$failures"'], capture_output=True, text=True)
        return result

    def test_stock_health_requires_profile_and_configured_instance(self):
        base = self.root / "mnt/data/nes-deck"
        (base / "langs").mkdir(parents=True)
        (base / "langs/python").symlink_to(sys.executable)
        (base / "install-bmc-scene.py").write_bytes(PROVISION.read_bytes())
        (base / "presentation").write_text("stock-bmc\n")
        self.provision()
        self.install()
        result = self.health()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("WIDGET-KEY ", result.stdout)
        (self.profile / "lib/bmc-widgets/retro-deck/manifest.json").unlink()
        self.assertNotEqual(self.health().returncode, 0)
        (base / "presentation").write_text("fork-bmc\n")
        self.assertNotEqual(self.health().returncode, 0)


if __name__ == "__main__":
    unittest.main()
