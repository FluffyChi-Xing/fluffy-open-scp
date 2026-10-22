"""OS-backed single-host lock for cooperating Frida capture entry points.

Serializes attach, arm, capture and teardown, including across checkouts.
Legacy PID-file owners are respected too. This does not control arbitrary
external injectors and does not prove that a previous agent has unloaded.
"""
from __future__ import annotations

import atexit
import ctypes
from ctypes import wintypes
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCK_PATH = ROOT / "tmp" / "dynamic" / "capture.lock"
MUTEX_NAME = r'Local\OpenSCP.SimCity.FridaCapture.v1'
_owned = False
_mutex = None


def _kernel32():
    k = ctypes.WinDLL('kernel32', use_last_error=True)
    k.CreateMutexW.argtypes = [ctypes.c_void_p, wintypes.BOOL, wintypes.LPCWSTR]
    k.CreateMutexW.restype = wintypes.HANDLE
    k.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    k.WaitForSingleObject.restype = wintypes.DWORD
    k.ReleaseMutex.argtypes = [wintypes.HANDLE]
    k.ReleaseMutex.restype = wintypes.BOOL
    k.CloseHandle.argtypes = [wintypes.HANDLE]
    k.CloseHandle.restype = wintypes.BOOL
    k.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    k.OpenProcess.restype = wintypes.HANDLE
    return k


def _alive(pid: int) -> bool:
    k = _kernel32()
    handle = k.OpenProcess(0x1000 | 0x100000, False, pid)
    if not handle:
        # Invalid PID is stale; access denied or any unexpected error is busy.
        return ctypes.get_last_error() != 87
    try:
        # Exited processes can still have queryable handles (e.g. their parent
        # retained a Popen object). Only a signaled process is known dead.
        return k.WaitForSingleObject(handle, 0) != 0
    finally:
        k.CloseHandle(handle)


def acquire_capture_lock() -> Path:
    """Acquire before attaching; reject contention without waiting."""
    global _owned, _mutex
    if _owned:
        raise RuntimeError('capture_lock_already_owned')
    if os.name != 'nt':
        raise RuntimeError('capture_lock_requires_windows')
    k = _kernel32()
    handle = k.CreateMutexW(None, False, MUTEX_NAME)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    result = k.WaitForSingleObject(handle, 0)
    if result not in (0, 0x80):  # acquired or abandoned by an exited host
        k.CloseHandle(handle)
        raise RuntimeError('capture_lock_busy' if result == 258 else 'capture_lock_wait_failed')
    try:
        LOCK_PATH.parent.mkdir(parents=True, exist_ok=True)
        if LOCK_PATH.exists():
            try:
                old = int(LOCK_PATH.read_text(encoding="ascii").strip())
            except (OSError, ValueError) as e:
                raise RuntimeError('capture_lock_owner_unreadable') from e
            if old <= 0 or (old != os.getpid() and _alive(old)):
                raise RuntimeError(f"capture_lock_busy:pid={old}:stop_existing_capture_first")
        LOCK_PATH.write_text(str(os.getpid()), encoding='ascii')
        _owned, _mutex = True, handle
        atexit.register(release_capture_lock)
        return LOCK_PATH
    except BaseException:
        k.ReleaseMutex(handle)
        k.CloseHandle(handle)
        raise


def release_capture_lock() -> None:
    global _owned, _mutex
    if not _owned:
        return
    _owned = False
    try:
        if int(LOCK_PATH.read_text(encoding="ascii").strip()) == os.getpid():
            LOCK_PATH.unlink(missing_ok=True)
    except (OSError, ValueError):
        pass
    finally:
        k = _kernel32()
        k.ReleaseMutex(_mutex)
        k.CloseHandle(_mutex)
        _mutex = None
