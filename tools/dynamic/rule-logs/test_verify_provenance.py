import copy
import unittest
from verify_provenance import verify


def fixture():
    common = {'channel': 'resource.provenance', 'schema': 1, 'requestId': 1, 'success': True,
              'database': {'pointer': '0x100', 'path': '测试.package'},
              'recordKey': {'instance': 3, 'type': 0x08068aeb, 'group': 0x40800200}}
    return [dict(common, phase='record', sequence=1, materialize=True, record='0x200'),
            dict(common, phase='create', sequence=2, record='0x200', resource='0x300'),
            dict(common, phase='request', sequence=3, resource='0x300', provenance='created-from-record',
                 key={'instance': 3, 'type': 0x08068ae9, 'group': 0x40800200})]


class ProvenanceTests(unittest.TestCase):
    def test_requires_all_three_events_and_exact_objects(self):
        self.assertTrue(verify(fixture())[0]['verified'])
        self.assertFalse(verify(fixture()[1:])[0]['verified'])
        rows = fixture()
        rows[1]['record'] = '0x201'
        self.assertFalse(verify(rows)[0]['verified'])

    def test_does_not_join_across_requests_or_guessed_cache_origin(self):
        rows = fixture()
        rows[-1]['requestId'] = 2
        self.assertFalse(any(r['verified'] for r in verify(rows)))
        rows = fixture()
        rows[-1].update(provenance='cache-hit-origin-unknown', resource='0x400', database=None)
        self.assertFalse(verify(rows)[0]['verified'])

    def test_rejects_duplicate_or_out_of_order_events(self):
        rows = fixture()
        with self.assertRaises(ValueError):
            verify(rows + [copy.deepcopy(rows[-1])])
        rows[0]['sequence'] = 10
        with self.assertRaises(ValueError):
            verify(rows)


if __name__ == '__main__':
    unittest.main()
