import unittest

from analyze_safari_inspector_trace import (
    percentile, summary_from_recording, timing_summary,
)


class InspectorAnalysisTests(unittest.TestCase):
    def test_nearest_rank_percentile_and_duration_threshold(self):
        self.assertEqual(percentile([0, 1, 2, 3, 9], .95), 9)
        self.assertIsNone(percentile([], .95))
        v = timing_summary([
            {"startTime": 0, "endTime": .019},
            {"startTime": .2, "endTime": .201},
        ])
        self.assertEqual(v["count"], 2)
        self.assertEqual(v["over_16_7_ms"], 1)

    def test_small_scrolling_capture_has_numeric_only_analysis(self):
        sample = {
            "version": 1,
            "recording": {
                "startTime": 0, "endTime": 4,
                "instrumentTypes": ["timeline-record-type-layout"],
                "records": [
                    {"type": "timeline-record-type-script", "details": "wheel",
                     "startTime": 1.0, "endTime": 1.001},
                    {"type": "timeline-record-type-script", "details": "wheel",
                     "startTime": 2.0, "endTime": 2.001},
                    {"type": "timeline-record-type-script", "details": "transitionrun",
                     "startTime": 1.3, "endTime": 1.301},
                    {"type": "timeline-record-type-layout", "eventType": "composite",
                     "startTime": 1.34, "endTime": 1.36},
                    {"type": "timeline-record-type-layout", "eventType": "composite",
                     "startTime": 1.7, "endTime": 1.702},
                    {"type": "timeline-record-type-screenshots", "timestamp": 1.3,
                     "imageData": "FAKE_PRIVATE_SCREENSHOT_CONTENT"},
                    {"type": "timeline-record-type-screenshots", "timestamp": 1.5,
                     "imageData": "FAKE_PRIVATE_SCREENSHOT_CONTENT"},
                ],
            }
        }
        out = summary_from_recording(sample)
        self.assertEqual(out["scroll"]["wheel_count"], 2)
        self.assertEqual(out["composite"]["during_scroll"]["count"], 2)
        self.assertEqual(out["composite"]["near_transition_within_100ms"]["count"], 1)
        self.assertEqual(out["composite"]["other_composites_during_scroll"]["count"], 1)
        self.assertEqual(out["screenshots"]["whole_recording"]["unique_payload_hashes"], 1)
        self.assertNotIn("FAKE_PRIVATE_SCREENSHOT_CONTENT", str(out))
        self.assertEqual(out["rendering_frame_record_count"], 0)

    def test_invalid_schema_or_non_scrolling_trace_rejected(self):
        with self.assertRaises(ValueError):
            summary_from_recording({"version": 2})
        with self.assertRaises(ValueError):
            summary_from_recording({"version": 1, "recording": {"records": []}})


if __name__ == "__main__":
    unittest.main()
