"""Cross-language QA protocol tests; no WebView, hardware or user data.

Exercise the actual JS marker encoder and Python CLI against disposable,
explicitly synthetic Safari-shaped records plus the read-only QA SQLite seam.
This verifies evidence plumbing, not native scrolling performance.
"""
import copy
from contextlib import closing
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
RUN_ID = "11111111111111111111111111111111"
OTHER_ID = "22222222222222222222222222222222"
PREFIX = "__ISSUE19_HOVER_QA__"


class TraceProtocolTests(unittest.TestCase):
    def test_cli_binds_js_markers_to_exact_sqlite_trial(self):
        encoder = subprocess.run(
            ["node", "--input-type=module", "-e", """
                import {phasePlan, phaseMarkerName} from './qa/native/issue19-hover-ab.mjs';
                const names = phasePlan().flatMap(({label}) =>
                  ['BEGIN', 'END'].map(side => phaseMarkerName(label, side, process.argv[1])));
                console.log(JSON.stringify(names));
            """, RUN_ID], cwd=ROOT, check=True, capture_output=True, text=True, timeout=15,
        )
        names = json.loads(encoder.stdout)
        records, phases = [], []
        for index, label in enumerate(("A1", "B1", "B2", "A2")):
            start = 2 + index * 10
            end = start + 6.16
            for name, time in zip(names[index*2:index*2+2], (start, end)):
                records.append({"details": name, "startTime": time, "endTime": time+.0001})
            for wheel in range(45):
                records.append({"details": "wheel", "startTime": start+.1+wheel*.12})
            transition_count = 12 if label[0] == "A" else 0
            for event in range(transition_count):
                records.append({"details": "transitionrun", "startTime": start+.2+event*.3})
            for event in range(8):
                time = start + .3 + event * .4
                records.append({"eventType": "composite", "startTime": time,
                                "endTime": time + (.024 if label[0] == "A" else .005)})
            phases.append({
                "phase": label, "mode": label[0], "valid": True,
                "start": {"pageSeconds": start}, "end": {"pageSeconds": end},
                "wheelEvents": 45, "trustedWheelEvents": 45, "travelPx": 1400,
                "rAFSamples": 55, "focusLost": False, "timedOut": False,
                "transitions": {"run": transition_count},
            })
        run = {"runId": RUN_ID, "inputMethod": "physical", "valid": True,
               "inputComparable": True, "startedAt": 1791810000000,
               "completedAt": 1791810050000, "phases": phases}
        trace = {"version": 1, "recording": {"records": records}}
        with (tempfile.TemporaryDirectory(prefix="tasknotes-hover-ab.",
                  dir=Path(tempfile.gettempdir()).resolve()) as database_dir,
              tempfile.TemporaryDirectory(prefix="tasknotes-protocol-test.") as workspace):
            db = Path(database_dir) / "tasknotes.db"
            with closing(sqlite3.connect(db)) as connection, connection:
                connection.execute("CREATE TABLE tasks(title TEXT, description TEXT)")
                connection.executemany("INSERT INTO tasks VALUES(?,?)", [
                    (f"{PREFIX}TASK-{i:02d}", "Synthetic protocol test") for i in range(31)
                ])
                connection.execute("INSERT INTO tasks VALUES(?,?)", (
                    PREFIX+"METRICS", json.dumps({"runs": [run]}),
                ))
            work = Path(workspace)
            marker = work / "src-tauri/target/.issue19-hover-data-dir"
            marker.parent.mkdir(parents=True)
            marker.write_text(database_dir)
            source, output = work / "synthetic.json", work / "numeric.json"
            command = [sys.executable, str(ROOT / "scripts/qa/compare_issue19_hover_inspector.py"),
                       str(source), "--require-physical-qa", "--output", str(output)]
            for identity in (RUN_ID, OTHER_ID, None):
                specimen = copy.deepcopy(trace)
                for record in specimen["recording"]["records"]:
                    name = record.get("details", "")
                    if name.startswith("ISSUE19_HOVER_"):
                        legacy_name = name.removesuffix("_" + RUN_ID)
                        record["details"] = legacy_name + ("_" + identity if identity else "")
                source.write_text(json.dumps(specimen))
                output.unlink(missing_ok=True)
                result = subprocess.run(command, cwd=work, capture_output=True,
                                        text=True, timeout=15)
                with self.subTest(identity=identity):
                    if identity == RUN_ID:
                        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                        report = json.loads(output.read_text())
                        self.assertTrue(report["qaBinding"]["qaTrustedWheelVerified"])
                        self.assertTrue(report["pairedMean"]["candidate"])
                        self.assertIn("CANDIDATE ONLY", result.stdout)
                    else:
                        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)
                        self.assertIn("trial identity", result.stdout)
                        self.assertFalse(output.exists())
            # The all-legacy fixture above is still useful for historical numeric
            # analysis, but must never be promoted to physical-input evidence.
            legacy = subprocess.run(command[:3], cwd=work, capture_output=True,
                                    text=True, timeout=15)
            self.assertEqual(legacy.returncode, 0, legacy.stdout + legacy.stderr)
            self.assertIn('"numericCandidate": true', legacy.stdout)
            self.assertIn('"qaTrustedWheelVerified": false', legacy.stdout)
            self.assertIn('"candidate": false', legacy.stdout)


if __name__ == "__main__":
    unittest.main()
