"""Exercise the actual QA runner without compiling or launching a native app."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class HoverBuildIsolationTests(unittest.TestCase):
    def test_qa_build_does_not_use_the_normal_or_inherited_cargo_target(self):
        with tempfile.TemporaryDirectory(prefix="tasknotes-build-isolation-") as directory:
            root = Path(directory).resolve()
            runner = root / "scripts/qa/run-issue19-hover-ab.sh"
            runner.parent.mkdir(parents=True)
            shutil.copy2(ROOT / "scripts/qa/run-issue19-hover-ab.sh", runner)
            commands = root / "fake-tools"
            commands.mkdir()
            # Stop at the external build boundary, before creating any bundle,
            # opening any database, invoking GUI tools or launching the app.
            (commands / "node").write_text("#!/bin/sh\nexit 0\n")
            (commands / "pnpm").write_text(
                '#!/bin/sh\nprintf "OBSERVED_TARGET=%s\\n" "${CARGO_TARGET_DIR-}"\nexit 31\n'
            )
            for command in commands.iterdir():
                command.chmod(0o700)
            inherited = root / "unexpected-inherited-output"
            result = subprocess.run(
                ["bash", str(runner)], cwd=root,
                env={**os.environ, "PATH": str(commands)+os.pathsep+os.environ["PATH"],
                     "CARGO_TARGET_DIR": str(inherited)},
                capture_output=True, text=True, timeout=15,
            )
            self.assertEqual(result.returncode, 31, result.stdout+result.stderr)
            observed = [line.removeprefix("OBSERVED_TARGET=") for line in result.stdout.splitlines()
                        if line.startswith("OBSERVED_TARGET=")]
            self.assertEqual(observed, [str(root / "src-tauri/target/issue19-hover-native")])
            self.assertFalse((root / "src-tauri/target/debug/tasknotes").exists())


    def test_reuse_refuses_legacy_shared_bundle_before_opening_any_data(self):
        with tempfile.TemporaryDirectory(prefix="tasknotes-build-isolation-") as directory:
            root = Path(directory).resolve()
            runner = root / "scripts/qa/run-issue19-hover-ab.sh"
            runner.parent.mkdir(parents=True)
            shutil.copy2(ROOT / "scripts/qa/run-issue19-hover-ab.sh", runner)
            legacy = root / "src-tauri/target/debug/bundle/macos/Tasknotes Issue19 Hover A B QA.app"
            legacy.mkdir(parents=True)
            result = subprocess.run(
                ["bash", str(runner), "--reuse-bundle"], cwd=root,
                capture_output=True, text=True, timeout=15,
            )
            self.assertEqual(result.returncode, 2, result.stdout+result.stderr)
            self.assertIn("Missing isolated QA bundle", result.stderr)
            self.assertFalse((root / "src-tauri/target/.issue19-hover-data-dir").exists())

if __name__ == "__main__":
    unittest.main()
