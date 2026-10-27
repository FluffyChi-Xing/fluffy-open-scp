"""Capture a game window with the Windows PrintWindow API.

This is a read-only capture path: it does not inject code or hook the game.
"""
import argparse
import ctypes
import ctypes.wintypes as wt
import os
from pathlib import Path

from PIL import Image, ImageGrab

user32 = ctypes.WinDLL("user32", use_last_error=True)
gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)

EnumWindows = user32.EnumWindows
EnumWindowsProc = ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)
GetWindowThreadProcessId = user32.GetWindowThreadProcessId
IsWindowVisible = user32.IsWindowVisible
GetClientRect = user32.GetClientRect
GetWindowRect = user32.GetWindowRect
PrintWindow = user32.PrintWindow
GetDC = user32.GetDC
ReleaseDC = user32.ReleaseDC
CreateCompatibleDC = gdi32.CreateCompatibleDC
CreateCompatibleBitmap = gdi32.CreateCompatibleBitmap
SelectObject = gdi32.SelectObject
DeleteObject = gdi32.DeleteObject
DeleteDC = gdi32.DeleteDC


class RECT(ctypes.Structure):
    _fields_ = [("left", wt.LONG), ("top", wt.LONG), ("right", wt.LONG), ("bottom", wt.LONG)]


def find_window(pid: int):
    found = []

    @EnumWindowsProc
    def callback(hwnd, _):
        owner = wt.DWORD()
        GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        if owner.value == pid and IsWindowVisible(hwnd):
            found.append(hwnd)
            return False
        return True

    EnumWindows(callback, 0)
    return found[0] if found else None


def capture(pid: int, output: Path):
    hwnd = find_window(pid)
    if not hwnd:
        raise RuntimeError("window_not_found")
    rect = RECT()
    if not GetClientRect(hwnd, ctypes.byref(rect)):
        raise RuntimeError("window_rect_failed")
    width, height = rect.right - rect.left, rect.bottom - rect.top
    if width < 32 or height < 32:
        raise RuntimeError("window_too_small")
    window_dc = GetDC(hwnd)
    memory_dc = CreateCompatibleDC(window_dc)
    bitmap = CreateCompatibleBitmap(window_dc, width, height)
    previous = SelectObject(memory_dc, bitmap)
    try:
        # PW_RENDERFULLCONTENT also captures modern compositor content when available.
        printed = bool(PrintWindow(hwnd, memory_dc, 0x00000002))
        bits = ctypes.create_string_buffer(width * height * 4)
        class BITMAPINFOHEADER(ctypes.Structure):
            _fields_ = [("size", wt.DWORD), ("width", wt.LONG), ("height", wt.LONG), ("planes", wt.WORD), ("bits", wt.WORD), ("compression", wt.DWORD), ("size_image", wt.DWORD), ("x", wt.LONG), ("y", wt.LONG), ("colors", wt.DWORD), ("important", wt.DWORD)]
        header = BITMAPINFOHEADER(ctypes.sizeof(BITMAPINFOHEADER), width, -height, 1, 32, 0, 0, 0, 0, 0, 0)
        gdi32.GetDIBits(memory_dc, bitmap, 0, height, bits, ctypes.byref(header), 0)
        image = Image.frombuffer("RGBA", (width, height), bits, "raw", "BGRA", 0, 1).copy()
        # DirectX/RenderWare often reports success but paints a black bitmap.
        # Fall back to a read-only desktop copy of the visible client area.
        extrema = image.convert("RGB").getextrema()
        if (not printed) or max(channel[1] for channel in extrema) < 4:
            window_rect = RECT()
            if not GetWindowRect(hwnd, ctypes.byref(window_rect)):
                raise RuntimeError("window_rect_failed")
            border_x = max(0, (window_rect.right - window_rect.left - width) // 2)
            title_h = max(0, (window_rect.bottom - window_rect.top - height) - border_x)
            bbox = (window_rect.left + border_x, window_rect.top + title_h,
                    window_rect.left + border_x + width, window_rect.top + title_h + height)
            try:
                image = ImageGrab.grab(bbox=bbox, all_screens=True).convert("RGBA")
            except Exception as error:
                if not printed:
                    raise RuntimeError("print_window_failed") from error
        output.parent.mkdir(parents=True, exist_ok=True)
        image.save(output, "PNG", optimize=True)
    finally:
        SelectObject(memory_dc, previous)
        DeleteObject(bitmap)
        DeleteDC(memory_dc)
        ReleaseDC(hwnd, window_dc)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    capture(args.pid, args.output)
