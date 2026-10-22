"""Window-boundary regression: dismissal must not vanish when clipping at 15h."""
import json
from pathlib import Path
import tempfile
import unittest

from analyze_school_timed_watch import analyze


class WindowTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name)
        (self.path/'status.json').write_text(json.dumps({'samples':3,'finishedUtc':'fixture'}))
        (self.path/'resource-map.json').write_text(json.dumps([[0x2C1DA112,0],[0x05DAADA6,1]]))
        rows=[]
        for tick,student in [(46559,950),(46560,0),(46561,0)]:
            rows.append(dict(utc='fixture',tickBefore=tick,tickAfter=tick,sameTick=True,stableIdentity=True,
                             bins=[dict(resourceIndex=0,amount=student),dict(resourceIndex=1,amount=950-student)],
                             periods=dict(ids=[0x12952639],times=[.625],inverseLengths=[1/1440],phases=[0],cycles=[32])))
        (self.path/'snapshots.jsonl').write_text('\n'.join(json.dumps(r) for r in rows))

    def test_dismissal_is_retained_but_predecessor_is_not_counted_in_window(self):
        result=analyze(self.path,46560,46560)
        self.assertEqual(result['samples'],1)
        self.assertEqual(result['resources']['Student']['first'],0)
        self.assertEqual(result['resources']['Student']['observedDecreases'],950)
        self.assertEqual(result['changes'][0]['sample'],1)
        self.assertEqual(result['windowSelection']['predecessorTick'],46559)
        self.assertTrue(result['statusSampleCountMatches'])

    def test_missing_window_reports_no_coverage(self):
        with self.assertRaisesRegex(ValueError,'empty_capture_window'):
            analyze(self.path,50000,50100)


if __name__=='__main__':unittest.main()
