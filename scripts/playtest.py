"""Human-like UI playtest harness for Unprompted: real mouse/keyboard via SendInput."""
import ctypes, time, os, sys, subprocess
from ctypes import wintypes
from PIL import Image

user32 = ctypes.windll.user32
user32.SetProcessDPIAware()
gdi32 = ctypes.windll.gdi32

PUL = ctypes.POINTER(ctypes.c_ulong)

class _INPUT(ctypes.Structure):
    _fields_ = [("w", wintypes.WORD), ("x", wintypes.WORD)]

class KEYBDINPUT(ctypes.Structure):
    _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD),
                ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", PUL)]

class MOUSEINPUT(ctypes.Structure):
    _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG), ("mouseData", wintypes.DWORD),
                ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD), ("dwExtraInfo", PUL)]

class INPUT(ctypes.Structure):
    class _I(ctypes.Union):
        _fields_ = [("ki", KEYBDINPUT), ("mi", MOUSEINPUT)]
    _anonymous_ = ("i",)
    _fields_ = [("type", wintypes.DWORD), ("i", _I)]

INP_MOUSE, INP_KEY = 0, 1
MOUSE_MOVE, MOUSE_ABS = 0x0001, 0x8000
MOUSE_LDOWN, MOUSE_LUP = 0x0002, 0x0004
KEY_UP = 0x0002

def move_to(x, y, steps=12):
    sw, sh = user32.GetSystemMetrics(0), user32.GetSystemMetrics(1)
    # current pos for smooth interpolation
    pt = wintypes.POINT(); user32.GetCursorPos(ctypes.byref(pt))
    for i in range(1, steps + 1):
        nx = pt.x + (x - pt.x) * i / steps
        ny = pt.y + (y - pt.y) * i / steps
        inp = INPUT(type=INP_MOUSE)
        inp.mi = MOUSEINPUT(int(nx * 65535 / (sw - 1)), int(ny * 65535 / (sh - 1)), 0, MOUSE_MOVE | MOUSE_ABS, 0, None)
        user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))
        time.sleep(0.012)
    time.sleep(0.08)

def click(x, y):
    move_to(x, y)
    down = INPUT(type=INP_MOUSE); down.mi = MOUSEINPUT(0, 0, 0, MOUSE_LDOWN, 0, None)
    up = INPUT(type=INP_MOUSE); up.mi = MOUSEINPUT(0, 0, 0, MOUSE_LUP, 0, None)
    user32.SendInput(1, ctypes.byref(down), ctypes.sizeof(INPUT))
    time.sleep(0.035)
    user32.SendInput(1, ctypes.byref(up), ctypes.sizeof(INPUT))
    time.sleep(0.15)

def key(vk, up=False, ext=0):
    inp = INPUT(type=INP_KEY)
    inp.ki = KEYBDINPUT(vk, 0, (KEY_UP if up else 0) | ext, 0, None)
    user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))

def type_text(text):
    for ch in text:
        # unicode via KEYEVENTF_UNICODE
        inp = INPUT(type=INP_KEY)
        inp.ki = KEYBDINPUT(0, ord(ch), 0x0004, 0, None)  # KEYEVENTF_UNICODE
        user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))
        up = INPUT(type=INP_KEY)
        up.ki = KEYBDINPUT(0, ord(ch), 0x0004 | KEY_UP, 0, None)
        user32.SendInput(1, ctypes.byref(up), ctypes.sizeof(INPUT))
        time.sleep(0.012)
    time.sleep(0.1)

def press(vk):  # tap
    key(vk); time.sleep(0.03); key(vk, up=True); time.sleep(0.08)

VK = {"RETURN": 0x0D, "ESC": 0x1B, "TAB": 0x09, "BACK": 0x08,
      "CTRL": 0x11, "O": 0x4F, "HOME": 0x24, "END": 0x23}

class RECTS(ctypes.Structure):
    _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long), ("right", ctypes.c_long), ("bottom", ctypes.c_long)]

def find_window(title="Unprompted"):
    return user32.FindWindowW(None, title)

def launch(exe, args=None, wait=5.0):
    cmd = [exe] + (args or [])
    p = subprocess.Popen(cmd, cwd=os.path.dirname(exe))
    time.sleep(wait)
    hwnd = find_window()
    if not hwnd:
        raise RuntimeError("window not found")
    # fixed position for predictable coordinates
    user32.MoveWindow(hwnd, 60, 40, 1062, 752, True)
    user32.SetForegroundWindow(hwnd)
    time.sleep(0.4)
    return p, hwnd

def window_rect(hwnd):
    r = RECTS()
    ctypes.windll.dwmapi.DwmGetWindowAttribute(hwnd, 9, ctypes.byref(r), ctypes.sizeof(RECTS))
    return r

def shot(hwnd, path):
    r = window_rect(hwnd)
    w, h = r.right - r.left, r.bottom - r.top
    hdc = user32.GetWindowDC(hwnd); mdc = gdi32.CreateCompatibleDC(hdc)
    bmp = gdi32.CreateCompatibleBitmap(hdc, w, h); gdi32.SelectObject(mdc, bmp)
    user32.PrintWindow(hwnd, mdc, 2)
    class BIH(ctypes.Structure):
        _fields_ = [("biSize", ctypes.c_uint32), ("biWidth", ctypes.c_long), ("biHeight", ctypes.c_long),
                    ("biPlanes", ctypes.c_ushort), ("biBitCount", ctypes.c_ushort),
                    ("biCompression", ctypes.c_uint32), ("biSizeImage", ctypes.c_uint32),
                    ("xPels", ctypes.c_long), ("yPels", ctypes.c_long),
                    ("clrUsed", ctypes.c_uint32), ("clrImportant", ctypes.c_uint32)]
    bi = BIH(ctypes.sizeof(BIH), w, -h, 1, 32, 0, 0, 0, 0, 0, 0)
    buf = ctypes.create_string_buffer(w * h * 4)
    gdi32.GetDIBits(mdc, bmp, 0, h, buf, ctypes.byref(bi), 0)
    Image.frombuffer("RGBA", (w, h), buf.raw, "raw", "BGRA", 0, 1).save(path)
    gdi32.DeleteObject(bmp); gdi32.DeleteDC(mdc); user32.ReleaseDC(hwnd, hdc)
    return w, h

def client_offset(hwnd):
    """Offset of the client area inside the window (title bar + borders)."""
    r = window_rect(hwnd)
    cr = RECTS()
    user32.GetClientRect(hwnd, ctypes.byref(cr))
    pt = wintypes.POINT(0, 0)
    user32.ClientToScreen(hwnd, ctypes.byref(pt))
    return pt.x - r.left, pt.y - r.top  # borderX, borderY(=titlebar)

def focus():
    user32.SetForegroundWindow(find_window())
    time.sleep(0.3)
