import copy
import unittest
from verify_effects import verify


def fixture():
    before = dict(pointer='0x100', resourceIndex=7, binIndex=2, amount=10, capacity=20)
    op = dict(ordinal=0, resourceId='0x00000005', input=0, output=1, status='observed',
              before=before, after=dict(before, amount=11), delta=1, expectedDeltaInt32=1, matchesInt32=True)
    return [dict(channel='simulation.effect', schema=1, sequence=1, applyCount=1,
                 nativeSuccess=True, readErrors=[], operations=[op])]


class EffectsTests(unittest.TestCase):
    def test_failure_return_does_not_erase_measured_write(self):
        rows = fixture()
        rows[0]['nativeSuccess'] = False
        self.assertTrue(verify(rows)[0]['verified'])
        self.assertEqual(verify(rows)[0]['delta'], 1)

    def test_rejects_tampered_derived_values_identity_and_read_errors(self):
        for mutate in [lambda r: r[0]['operations'][0].update(delta=9),
                       lambda r: r[0]['operations'][0]['after'].update(pointer='0x200'),
                       lambda r: r[0].update(readErrors=['fault'])]:
            rows = fixture()
            mutate(rows)
            self.assertFalse(verify(rows)[0]['verified'])

    def test_unobserved_operations_are_not_proven_zero_changes(self):
        rows = fixture()
        rows[0]['operations'][0] = dict(status='not-reached')
        self.assertFalse(verify(rows)[0]['verified'])

    def test_duplicate_events_rejected(self):
        rows = fixture()
        with self.assertRaises(ValueError):
            verify(rows + copy.deepcopy(rows))


if __name__ == '__main__':
    unittest.main()
