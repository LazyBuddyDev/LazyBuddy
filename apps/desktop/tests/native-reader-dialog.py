"""Complete only the test host's own file dialog, without physical input."""
import ctypes
from ctypes import wintypes as w
import json
import os
import sys
import time

pid = int(sys.argv[1])
cancel = sys.argv[2] == "--cancel"
file_path = os.path.abspath(sys.argv[2]) if not cancel else ""
user = ctypes.WinDLL("user32", use_last_error=True)
user.SendMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user.SendMessageW.restype = w.LPARAM
callback_type = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
deadline = time.monotonic() + (1 if cancel else 15)
while time.monotonic() < deadline:
    dialogs = []

    @callback_type
    def visit(hwnd, _):
        owner = w.DWORD()
        user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
        name = ctypes.create_unicode_buffer(64)
        user.GetClassNameW(hwnd, name, len(name))
        if owner.value == pid and name.value == "#32770" and user.IsWindowVisible(hwnd):
            dialogs.append(hwnd)
        return True

    user.EnumWindows(visit, 0)
    if dialogs:
        if cancel:
            user.PostMessageW(dialogs[0], 0x111, 2, 0)
            print(json.dumps({"pid": pid, "cancelled_owned_dialog": True}))
            break
        selected = ctypes.create_unicode_buffer(file_path)
        edits = []

        @callback_type
        def filename_control(hwnd, _):
            name = ctypes.create_unicode_buffer(64)
            user.GetClassNameW(hwnd, name, len(name))
            if name.value == "Edit" and user.GetDlgCtrlID(hwnd) in (1148, 1152):
                edits.append(hwnd)
            return True

        user.EnumChildWindows(dialogs[0], filename_control, 0)
        if not edits:
            raise RuntimeError("Filename control not found in owned dialog")
        user.SendMessageW(edits[0], 0xC, 0, ctypes.addressof(selected))
        user.PostMessageW(dialogs[0], 0x111, 1, 0)
        print(json.dumps({"pid": pid, "selected_owned_dialog": True}))
        break
    time.sleep(0.05)
else:
    if not cancel:
        raise RuntimeError("Owned reader file dialog did not appear")
