"""Regression for normal event-rule growth that interrupted the first watch."""
import struct
import unittest

from school_timed_watch import validate_core_binding


class CoreBindingTests(unittest.TestCase):
    def test_event_append_and_reallocation_do_not_change_core_binding(self):
        core=struct.pack('<3I',30695,7069,5150)
        validate_core_binding(core,bytes(bytearray(core)),3)
        validate_core_binding(core,core+struct.pack('<2I',5165,5154),5)

    def test_changed_or_removed_core_rule_stops_capture(self):
        core=struct.pack('<3I',30695,7069,5150)
        for changed in (struct.pack('<3I',6728,7069,5150),core[:8]):
            with self.assertRaisesRegex(ValueError,'core_rule_binding_changed'):
                validate_core_binding(core,changed,len(changed)//4)

    def test_transitional_array_lengths_are_not_accepted(self):
        core=struct.pack('<3I',30695,7069,5150)
        with self.assertRaisesRegex(ValueError,'rule_vectors_in_transition'):
            validate_core_binding(core,core+struct.pack('<I',5165),3)


if __name__=='__main__':unittest.main()
