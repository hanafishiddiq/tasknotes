import unittest
from contextlib import closing

from compare_issue19_hover_inspector import trace_phases,summarize,trace_within_size_limit,verify_physical_qa,latest_qa_run

class HoverTraceComparisonTests(unittest.TestCase):

    def valid_physical_pair(self):
        result=summarize(self.fixture(),trace_phases(self.fixture()))
        run={"runId":"11111111111111111111111111111111",
             "inputMethod":"physical","valid":True,"inputComparable":True,
             "startedAt":1791810000000,"completedAt":1791810050000,"phases":[]}
        for i,(row,label) in enumerate(zip(result["phases"],("A1","B1","B2","A2"))):
            row["runId"]=run["runId"]
            row["durationSeconds"]=6.160
            row["syntheticWheelEvents"]=45
            run["phases"].append({
                "phase":label,"mode":label[0],"valid":True,
                "start":{"pageSeconds":100+i*10},
                "end":{"pageSeconds":106.160+i*10},
                "wheelEvents":45,"trustedWheelEvents":45,"travelPx":1400,
                "rAFSamples":55,"focusLost":False,"timedOut":False,
                "transitions":{"run":12 if label.startswith("A") else 0},
            })
        return result,run

    def test_all_phase_markers_carry_one_trial_identity(self):
        run_id = "11111111111111111111111111111111"
        records = self.fixture()
        for record in records:
            if record.get("details", "").startswith("ISSUE19_HOVER_"):
                record["details"] += "_" + run_id
        phases = trace_phases(records)
        self.assertEqual([p.get("runId") for p in phases], [run_id] * 4)
        self.assertEqual([p.get("runId") for p in summarize(records, phases)["phases"]], [run_id] * 4)
        for replacement in ("ISSUE19_HOVER_A1_BEGIN",
                            "ISSUE19_HOVER_A1_BEGIN_22222222222222222222222222222222"):
            mixed = [dict(r) for r in records]
            mixed[0]["details"] = replacement
            with self.subTest(replacement=replacement), self.assertRaisesRegex(ValueError, "trial identity"):
                trace_phases(mixed)

    def test_physical_qa_rejects_different_trial_with_matching_statistics(self):
        numeric, run = self.valid_physical_pair()
        run["runId"] = "22222222222222222222222222222222"
        for phase in numeric["phases"]:
            phase["runId"] = "11111111111111111111111111111111"
        with self.assertRaisesRegex(ValueError, "trial identity"):
            verify_physical_qa(numeric, run)

    def test_physical_qa_binding_accepts_fully_matched_run_as_candidate_only(self):
        numeric,run=self.valid_physical_pair()
        verified=verify_physical_qa(numeric,run)
        self.assertTrue(verified["qaTrustedWheelVerified"])
        self.assertTrue(verified["baselineRepeatedSlowComposites"])
        self.assertTrue(verified["candidate"])
        self.assertNotIn("physicalDisplayFps",verified)

    def test_physical_qa_binding_rejects_scripted_and_unfinished_runs(self):
        numeric,run=self.valid_physical_pair()
        for bad in ({"inputMethod":"scripted"},{"valid":False},
                    {"inputComparable":False},{"phases":[]},
                    {"completedAt":None}):
            with self.subTest(bad=bad),self.assertRaisesRegex(ValueError,"INCONCLUSIVE"):
                verify_physical_qa(numeric,{**run,**bad})

    def test_physical_qa_binding_rejects_false_trust_or_incomplete_focus(self):
        numeric,run=self.valid_physical_pair()
        for field,value in (("trustedWheelEvents",0),("focusLost",True),
                            ("valid",False),("travelPx",1),("rAFSamples",0)):
            bad={**run,"phases":[dict(p) for p in run["phases"]]}
            bad["phases"][1][field]=value
            with self.subTest(field=field),self.assertRaisesRegex(ValueError,"INCONCLUSIVE"):
                verify_physical_qa(numeric,bad)

    def test_physical_qa_binding_rejects_mismatched_timelines_and_css(self):
        numeric,run=self.valid_physical_pair()
        for field,value in (("syntheticWheelEvents",12),("durationSeconds",9.5),
                            ("transitionRuns",0)):
            altered={**numeric,"phases":[dict(p) for p in numeric["phases"]]}
            altered["phases"][0][field]=value
            with self.subTest(field=field),self.assertRaisesRegex(ValueError,"INCONCLUSIVE"):
                verify_physical_qa(altered,run)
        altered={**numeric,"phases":[dict(p) for p in numeric["phases"]]}
        altered["phases"][1]["transitionRuns"]=5
        with self.assertRaisesRegex(ValueError,"INCONCLUSIVE"):
            verify_physical_qa(altered,run)

    def test_physical_qa_binding_rejects_imbalanced_wheel_workload(self):
        numeric,run=self.valid_physical_pair()
        bad={**run,"phases":[dict(p) for p in run["phases"]]}
        bad["phases"][1]["trustedWheelEvents"]=150
        bad["phases"][1]["wheelEvents"]=150
        with self.assertRaisesRegex(ValueError,"INCONCLUSIVE"):
            verify_physical_qa(numeric,bad)

    def test_compositing_candidate_requires_both_a_baselines_red(self):
        numeric,run=self.valid_physical_pair()
        altered={**numeric,"phases":[dict(p) for p in numeric["phases"]]}
        altered["phases"][3]["compositeSlowOver16_7"]=0
        verified=verify_physical_qa(altered,run)
        self.assertTrue(verified["qaTrustedWheelVerified"])
        self.assertFalse(verified["baselineRepeatedSlowComposites"])
        self.assertFalse(verified["candidate"])

    def test_synthetic_existing_trace_cannot_be_approved_as_physical(self):
        numeric,run=self.valid_physical_pair()
        run["inputMethod"]="scripted"
        with self.assertRaisesRegex(ValueError,"INCONCLUSIVE"):
            verify_physical_qa(numeric,run)


    def test_qa_sqlite_reader_uses_only_isolated_fixture_and_latest_run(self):
        import json
        import sqlite3
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory(prefix="tasknotes-hover-ab.",
                                         dir=Path(tempfile.gettempdir()).resolve()) as directory:
            db=Path(directory)/"tasknotes.db"
            with closing(sqlite3.connect(db)) as conn, conn:
                conn.execute("CREATE TABLE tasks(title TEXT, description TEXT)")
                conn.executemany("INSERT INTO tasks VALUES(?,?)",[
                    (f"__ISSUE19_HOVER_QA__TASK-{i:02d}", "synthetic") for i in range(31)
                ])
                conn.execute("INSERT INTO tasks VALUES(?,?)",(
                    "__ISSUE19_HOVER_QA__METRICS",
                    json.dumps({"runs":[{"inputMethod":"scripted"},{"inputMethod":"physical"}]})
                ))
            with tempfile.TemporaryDirectory(prefix="qa-marker-") as markers:
                marker=Path(markers)/"marker"
                marker.write_text(str(directory))
                self.assertEqual(latest_qa_run(marker)["inputMethod"],"physical")
                with closing(sqlite3.connect(db)) as conn, conn:
                    conn.execute("UPDATE tasks SET title='private data' WHERE title LIKE '%TASK-00'")
                with self.assertRaisesRegex(ValueError,"INCONCLUSIVE"):
                    latest_qa_run(marker)
                marker.write_text("/tmp/non-qa-data")
                with self.assertRaisesRegex(ValueError,"INCONCLUSIVE"):
                    latest_qa_run(marker)

    def test_accepts_large_but_bounded_inspector_export(self):
        # No fixture bytes / user trace needed; 256 MiB is the fixed memory-safety ceiling.
        self.assertTrue(trace_within_size_limit(250_304_955))
        self.assertTrue(trace_within_size_limit(256 * 1024 * 1024))
        self.assertFalse(trace_within_size_limit(256 * 1024 * 1024 + 1))
        self.assertFalse(trace_within_size_limit(0))

    def fixture(self):
        records=[]
        for number,label in enumerate(("A1","B1","B2","A2")):
            start=2+number*6
            end=start+4
            for name,time in (("BEGIN",start),("END",end)):
                records.append({"type":"timeline-record-type-script",
                                "details":f"ISSUE19_HOVER_{label}_{name}",
                                "startTime":time,"endTime":time+.0001})
            for i in range(25):
                records.append({"type":"timeline-record-type-script","details":"wheel",
                                "startTime":start+.03+i*.12,"endTime":start+.03+i*.12+.0001})
            if label.startswith("A"):
                for i in range(12):
                    records.append({"type":"timeline-record-type-script",
                                    "details":"transitionrun","startTime":start+.05+i*.21,
                                    "endTime":start+.05+i*.21+.001})
            for i in range(8):
                composite_ms = 24 if label.startswith("A") else 5
                time=start+.06+i*.3
                records.append({"type":"timeline-record-type-layout","eventType":"composite",
                                "startTime":time,"endTime":time+composite_ms/1000})
        return records

    def test_segment_a_b_b_a_and_reject_duplicate_markers(self):
        records=self.fixture()
        windows=trace_phases(records)
        self.assertEqual([p["label"] for p in windows],["A1","B1","B2","A2"])
        self.assertEqual([p["durationSeconds"] for p in windows],[4,4,4,4])
        with self.assertRaisesRegex(ValueError,"duplicate"):
            trace_phases(records+[records[0]])

    def test_synthetic_expensive_compositing_candidate_math(self):
        records=self.fixture()
        profile=summarize(records,trace_phases(records))
        self.assertEqual([p["transitionRuns"] for p in profile["phases"]],[12,0,0,12])
        self.assertEqual([p["syntheticWheelEvents"] for p in profile["phases"]],[25]*4)
        self.assertEqual(profile["pairedMean"]["p95ReductionPercent"],79.2)
        self.assertEqual(profile["pairedMean"]["slowReductionPercent"],100)
        self.assertTrue(profile["pairedMean"]["candidate"])

    def test_fast_baseline_is_not_a_performance_fix_candidate(self):
        records=self.fixture()
        for row in records:
            if row.get("eventType")=="composite":
                row["endTime"]=row["startTime"]+(.010 if row["startTime"] < 8 or row["startTime"] > 20 else .003)
        result=summarize(records,trace_phases(records))
        self.assertGreater(result["pairedMean"]["p95ReductionPercent"],30)
        self.assertFalse(result["pairedMean"]["baselineExpensiveCompositingPresent"])
        self.assertFalse(result["pairedMean"]["candidate"])

    def test_no_improvement_never_falsely_claims_candidate(self):
        records=self.fixture()
        for row in records:
            if row.get("eventType")=="composite":
                row["endTime"]=row["startTime"]+.024
        result=summarize(records,trace_phases(records))
        self.assertFalse(result["pairedMean"]["candidate"])
        self.assertEqual(result["pairedMean"]["p95ReductionPercent"],0)

    def test_missing_markers_are_inconclusive(self):
        records=[r for r in self.fixture() if r.get("details")!="ISSUE19_HOVER_B2_END"]
        with self.assertRaisesRegex(ValueError,"INCONCLUSIVE"):
            trace_phases(records)

if __name__=="__main__":
    unittest.main()
