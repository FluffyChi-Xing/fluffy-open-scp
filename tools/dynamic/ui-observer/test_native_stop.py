import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch
import native_trace

class StopTest(unittest.TestCase):
    def test_stop_request_unloads_and_detaches(self):
        with tempfile.TemporaryDirectory() as root:
            directory = Path(root) / 'capture'
            directory.mkdir()
            directory.with_suffix('.stop').write_text('stop')
            script, session = MagicMock(), MagicMock()
            session.create_script.return_value = script
            frida = MagicMock()
            frida.__version__ = 'test'
            frida.get_local_device.return_value.attach.return_value = session
            with patch.dict('sys.modules', {'frida': frida}):
                result = native_trace.capture(123, {'seconds': 60}, directory)
            script.exports_sync.stop.assert_called_once()
            script.unload.assert_called_once()
            session.detach.assert_called_once()
            self.assertEqual(result['stopReason'], 'requested')
            self.assertTrue(result['unloaded'])
            self.assertTrue(result['detached'])

if __name__ == '__main__':
    unittest.main()
