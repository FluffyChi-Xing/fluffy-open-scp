import ctypes as c
import struct
import unittest
from unittest.mock import patch
import eawebkit_runtime as runtime

class ScanTest(unittest.TestCase):
    def test_view_after_old_byte_limit_is_found(self):
        class Reader:
            handle = 1
            def __init__(self): self.k = self
            def VirtualQueryEx(self, handle, address, pointer, size):
                region = c.cast(pointer, c.POINTER(runtime.MemoryRegion)).contents
                region.BaseAddress = address
                region.State, region.Type, region.Protect = 0x1000, 0x20000, 4
                region.RegionSize = (0xf0000000 if address == 0 else 0x100000000) - address
                return size
            def read(self, address, size):
                return struct.pack('<I', 0x85C2B8) if address == 0xf0000000 else b''
        with patch.object(runtime, 'validate_view', return_value={'view': '0xf0000000'}):
            result = runtime.scan_views(Reader(), 0)
        self.assertEqual(result['candidates'], [{'view': '0xf0000000'}])
        self.assertGreater(result['scannedBytes'], 1536 * 1048576)
        self.assertEqual(result['stopReason'], 'complete')

    def test_deadline_reports_incomplete(self):
        with patch.object(runtime.time, 'monotonic', side_effect=[0,21,21,21]):
            result = runtime.scan_views(None, 0)
        self.assertEqual(result['stopReason'], 'time_limit')
        self.assertFalse(result['candidates'])

if __name__ == '__main__': unittest.main()
