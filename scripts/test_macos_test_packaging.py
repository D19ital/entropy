#!/usr/bin/env python3
"""Change-scoped shell regression test; hdiutil and sleep are MOCKED, no app build."""
from pathlib import Path
import json
import os
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = (ROOT / "scripts/build_macos_test_app.sh").read_text()
FUNCTION = SCRIPT.split("create_dmg_with_retries() {", 1)[1].split(
    "\nmacos_validate_stable_signing_configuration", 1
)[0]
FUNCTION = "create_dmg_with_retries() {" + FUNCTION


class DmgRetries(unittest.TestCase):
    def run_mock(self, status):
        with tempfile.TemporaryDirectory(prefix="entropy-dmg-mock-") as tmp:
            program = FUNCTION + r'''
set -eu
APP_NAME=Entropy
APP_PATH="$MOCK_ROOT/Entropy.app"
DMG_PATH="$MOCK_ROOT/test.dmg"
sleep() { :; }
hdiutil() { printf 'called\n' >> "$MOCK_ROOT/calls"; return "$MOCK_STATUS"; }
create_dmg_with_retries
'''
            result = subprocess.run(
                ["bash", "-c", program], text=True, capture_output=True,
                env={**os.environ, "MOCK_ROOT": tmp, "MOCK_STATUS": str(status)},
            )
            calls = (Path(tmp) / "calls").read_text().splitlines()
            return result, calls

    def test_success_does_not_retry(self):
        result, calls = self.run_mock(0)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(calls), 1)

    def test_three_failures_preserve_failure_status(self):
        result, calls = self.run_mock(19)
        self.assertEqual(result.returncode, 19, result.stdout + result.stderr)
        self.assertEqual(len(calls), 3)


class Cleanup(unittest.TestCase):
    def test_restore_search_list_with_spaces_and_remove_dummy_material(self):
        # MOCK security/uname; these files are inert text, never keys or certificates.
        with tempfile.TemporaryDirectory(prefix="entropy-cleanup-mock-") as tmp:
            directory = Path(tmp)
            mocks = directory / "bin"
            mocks.mkdir()
            for name, body in {
                "uname": "print('Darwin')\n",
                "security": (
                    "import json, os, sys\n"
                    "with open(os.environ['MOCK_LOG'], 'a') as log:\n"
                    "    log.write(json.dumps(sys.argv[1:]) + '\\n')\n"
                ),
            }.items():
                executable = mocks / name
                executable.write_text(f"#!{sys.executable}\n{body}")
                executable.chmod(0o755)
            original = ["/Users/test/Library/Keychains/login.keychain-db", "/path with spaces/custom.keychain-db"]
            (directory / "keychains.original").write_text(
                "".join(f'    "{path}"\n' for path in original)
            )
            for name in ("certificate.p12", "keychain.password", "signing.keychain-db"):
                (directory / name).write_text("DUMMY NON-SECRET TEXT")
            log = directory / "security.log"
            result = subprocess.run(
                ["bash", str(ROOT / "scripts/sign_macos_test_pair.sh"), "cleanup"],
                env={**os.environ, "TEST_ROOT": tmp, "MOCK_LOG": str(log),
                     "PATH": f"{mocks}{os.pathsep}{os.environ['PATH']}"},
                text=True, capture_output=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            calls = [json.loads(line) for line in log.read_text().splitlines()]
            self.assertEqual(calls[0], ["list-keychains", "-d", "user", "-s", *original])
            self.assertEqual(calls[1], ["delete-keychain", str(directory / "signing.keychain-db")])
            for name in ("certificate.p12", "keychain.password", "signing.keychain-db"):
                self.assertFalse((directory / name).exists(), name)


if __name__ == "__main__":
    unittest.main(verbosity=2)
