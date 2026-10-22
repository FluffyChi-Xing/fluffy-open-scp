"""Windows subprocess tests. No Frida import or game process access."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import uuid

CHILD = '''
import sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
import capture_lock as lock
lock.LOCK_PATH = Path(sys.argv[2])
lock.MUTEX_NAME = sys.argv[3]
try:
    lock.acquire_capture_lock()
except RuntimeError as e:
    print(str(e), flush=True)
    sys.exit(7)
print('acquired', flush=True)
if sys.argv[4] == 'hold':
    sys.stdin.readline()
lock.release_capture_lock()
'''


@unittest.skipUnless(os.name == 'nt', 'Windows mutex tests')
class CaptureLockTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / 'capture.lock'
        self.name = 'Local\\OpenSCP.CaptureLockTest.' + uuid.uuid4().hex

    def command(self, mode):
        return [sys.executable, '-c', CHILD, str(Path(__file__).parent), str(self.path), self.name, mode]

    def run_once(self):
        return subprocess.run(self.command('once'), capture_output=True, text=True, timeout=10)

    def test_live_owner_blocks_and_clean_release_allows(self):
        with subprocess.Popen(self.command('hold'), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, text=True) as owner:
            try:
                # Metadata is written before acquiring is reported. A separate
                # process's exit code proves the OS lock actually excludes it.
                self.assertEqual(owner.stdout.readline().strip(), 'acquired')
                denied = self.run_once()
                self.assertEqual(denied.returncode, 7, denied.stderr)
                self.assertIn('capture_lock_busy', denied.stdout)
                owner.communicate('\n', timeout=10)
            finally:
                if owner.poll() is None:
                    owner.kill()
                    owner.wait(timeout=10)
        self.assertEqual(self.run_once().returncode, 0)
        self.assertFalse(self.path.exists())

    def test_exited_host_releases_mutex_and_stale_pid_is_reclaimed(self):
        with subprocess.Popen(self.command('hold'), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, text=True) as owner:
            try:
                self.assertEqual(owner.stdout.readline().strip(), 'acquired')
            finally:
                owner.kill()  # Only the disposable test child, never SimCity.
                owner.wait(timeout=10)
        self.assertTrue(self.path.exists())
        self.assertEqual(self.run_once().returncode, 0)

    def test_legacy_live_pid_is_respected(self):
        self.path.write_text(str(os.getpid()), encoding='ascii')
        self.assertEqual(self.run_once().returncode, 7)
        self.assertEqual(self.path.read_text(), str(os.getpid()))

    def test_malformed_owner_fails_closed(self):
        self.path.write_text('', encoding='ascii')
        result = self.run_once()
        self.assertEqual(result.returncode, 7)
        self.assertIn('owner_unreadable', result.stdout)


if __name__ == '__main__':
    unittest.main()
