import copy
import unittest
from verify_conditions import verify


def fixture():
    return [dict(channel='simulation.condition', schema=1, sequence=1, readErrors=[], nativeAllowed=False,
                 finalApplyCount=0, control=dict(actual=[0, 0], mask=[0, 0], expected=[0, 0], matches=True),
                 countCapture='captured', resourceCount=0, minApply=1, configuredMaxApply=1,
                 applyResourceId='0x00000000', diagnosis=dict(reason='capacity-shortfall', ordinal=0),
                 operations=[dict(ordinal=0, status='observed', input=0, output=1, bin=dict(amount=24, capacity=24))])]


class ConditionsTests(unittest.TestCase):
    def test_requires_snapshot_native_count_and_diagnosis_agreement(self):
        self.assertTrue(verify(fixture())[0]['verified'])
        for change in [lambda r: r[0].update(resourceCount=1),
                       lambda r: r[0]['operations'][0]['bin'].update(amount=23),
                       lambda r: r[0]['diagnosis'].update(reason='input-shortfall'),
                       lambda r: r[0]['control'].update(matches=False)]:
            rows = fixture()
            change(rows)
            self.assertFalse(verify(rows)[0]['verified'])

    def test_control_rejection_does_not_require_resource_count(self):
        rows = fixture()
        rows[0].update(control=dict(actual=[4, 0], mask=[4, 0], expected=[0, 0], matches=False),
                       countCapture='not-observed', operations=[], diagnosis=dict(reason='control-flags'))
        self.assertTrue(verify(rows)[0]['verified'])

    def test_minimum_count_and_zero_maximum(self):
        rows = fixture()
        r = rows[0]
        r.update(resourceCount=2, minApply=3, configuredMaxApply=0, diagnosis=dict(reason='minimum-apply'))
        r['operations'][0].update(input=2, output=0, bin=dict(amount=4, capacity=10))
        self.assertTrue(verify(rows)[0]['verified'])
        r.update(minApply=1, nativeAllowed=True, finalApplyCount=2, diagnosis=dict(reason='allowed'))
        self.assertTrue(verify(rows)[0]['verified'])

    def test_errors_and_duplicate_rows_are_not_verified(self):
        rows = fixture()
        rows[0]['readErrors'] = ['fault']
        self.assertFalse(verify(rows)[0]['verified'])
        with self.assertRaises(ValueError):
            verify(rows + copy.deepcopy(rows))


if __name__ == '__main__':
    unittest.main()
