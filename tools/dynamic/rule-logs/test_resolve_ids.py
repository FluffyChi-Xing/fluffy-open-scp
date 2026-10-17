import struct
import unittest
from resolve_ids import resolve, resource_ops


def words(*values):
    return b''.join(struct.pack('>I', v) for v in values)


def fixture():
    # Two unit records sharing an ID: resolver must retain ambiguity.
    unit = words(0x12345678) + bytes(128) + words(0) + bytes(8)
    glob = words(0x87654321) + bytes(100)
    return words(5, 2, 14) + b'\0' + words(2) + unit * 2 + words(5, 0, 8, 1) + glob + words(2, 0, 0, 0, 0, 0)


class ResolveTests(unittest.TestCase):
    def test_resource_ops_use_signed_little_endian_pool_and_bound_output(self):
        pool = struct.pack('<Iii', 0x98ec1f24, 4, -12) * 17
        result = resource_ops(pool, 0, 17)
        self.assertEqual(result['values'][0], dict(resourceId='0x98ec1f24', input=4, output=-12))
        self.assertEqual(len(result['values']), 16)
        self.assertTrue(result['truncated'])
        self.assertEqual(resource_ops(b'', -1, 0)['values'], [])

    def test_resource_ops_reject_invalid_full_ranges_before_truncation(self):
        for offset, count in [(-1, 1), (0, -1), (1, 1), (0, 17), (2**31, 1)]:
            with self.assertRaises(ValueError):
                resource_ops(bytes(12), offset, count)

    def test_preserves_duplicate_ids_and_scopes(self):
        result = resolve(fixture(), {0x12345678, 0x87654321})
        self.assertEqual([(x['scope'], x['index']) for x in result['matches']], [('unit', 0), ('unit', 1), ('global', 0)])
        self.assertTrue(result['exact'])

    def test_rejects_partial_or_trailing_data(self):
        for data in [fixture()[:-1], fixture() + b'\0']:
            with self.assertRaises(ValueError):
                resolve(data, {0x12345678})

    def test_rejects_unverified_layout_and_names(self):
        for data in [words(4) + fixture()[4:], fixture()[:12] + b'\1' + fixture()[13:]]:
            with self.assertRaises(ValueError):
                resolve(data, set())


if __name__ == '__main__':
    unittest.main()
