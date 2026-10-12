import unittest

from compare_issue19_hover_inspector import trace_phases,summarize,trace_within_size_limit

class HoverTraceComparisonTests(unittest.TestCase):
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
