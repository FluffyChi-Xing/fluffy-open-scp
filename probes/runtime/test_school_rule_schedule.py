"""Bounds tests and regressions against the actual locally extracted ERZ."""
import struct
import unittest
from pathlib import Path

from school_rule_schedule import sim_info, unit_prop_sets
from unit_rule_resource_bridge import decode


class ScheduleBounds(unittest.TestCase):
    def test_absent_siminfo_is_distinct_from_pool_zero(self):
        self.assertTrue(sim_info(b'', -1)['default'])
        for offset in (-2, 0, 64):
            with self.assertRaisesRegex(ValueError, 'sim_info_bounds'):
                sim_info(bytes(67), offset)

    def test_version_and_truncation_rejected(self):
        with self.assertRaises(ValueError):
            unit_prop_sets(struct.pack('>I', 6))
        with self.assertRaises(ValueError):
            unit_prop_sets(b'')


@unittest.skipUnless(Path('tmp/live-school-installed-main.bin').exists(), 'local game fixture unavailable')
class InstalledSchedule(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = Path('tmp/live-school-installed-main.bin').read_bytes()
        cls.decoded = decode(cls.data, {0x23E737BD, 0xB52C889D, 0xC193A5BB, 0xD51CB94C, 0xD54F8B18})
        if cls.decoded['sha256'] != 'a388e3c15080a4721e7d0047da180f6ee9f38292060c67892fa34b952564c381':
            raise unittest.SkipTest('different game fixture')
        cls.rules = {r['ruleId']: r for r in cls.decoded['matches']}
        cls.pool = cls.data[cls.decoded['poolOffset']:]

    def info(self, rid):
        offset = struct.unpack_from('>i', self.data, self.rules[rid]['offset'] + 8)[0]
        return sim_info(self.pool, offset)

    def test_hourly_rule_and_six_am_reset(self):
        hourly = self.info(0x23E737BD)
        self.assertEqual((hourly['triggerPeriodId'], hourly['triggerTime']), ('0xd5a08f93', 0))
        daily = self.info(0xC193A5BB)
        self.assertEqual((daily['triggerPeriodId'], daily['triggerTime']), ('0x12952639', 0.25))
        self.assertTrue(self.info(0xB52C889D)['default'])

    def test_power_base_interval_is_overridden_by_school_property(self):
        self.assertEqual(self.info(0xD51CB94C)['rate'], 30)
        props = unit_prop_sets(self.data)
        power = props[0xD51CB94C][0]
        water = props[0xD54F8B18][0]
        self.assertEqual((power['propertyId'], power['opId']), ('0x0afb9882', 0))
        self.assertEqual((water['propertyId'], water['opId']), ('0x0c09da6a', 0))
        self.assertEqual((power['multiplier'], power['addend']), (1, 0))


if __name__ == '__main__':
    unittest.main()
