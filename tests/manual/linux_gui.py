#!/usr/bin/env python3
"""Exercise only an isolated brows instance and synthetic Vivaldi on XWayland."""

import argparse
import ctypes as c
import ctypes.util
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import time

from PIL import Image


class WindowAttributes(c.Structure):
    _fields_ = [
        ("x", c.c_int), ("y", c.c_int), ("width", c.c_int), ("height", c.c_int),
        ("border_width", c.c_int), ("depth", c.c_int), ("visual", c.c_void_p),
        ("root", c.c_ulong), ("window_class", c.c_int), ("bit_gravity", c.c_int),
        ("win_gravity", c.c_int), ("backing_store", c.c_int),
        ("backing_planes", c.c_ulong), ("backing_pixel", c.c_ulong),
        ("save_under", c.c_int), ("colormap", c.c_ulong), ("map_installed", c.c_int),
        ("map_state", c.c_int), ("all_event_masks", c.c_long),
        ("your_event_mask", c.c_long), ("do_not_propagate_mask", c.c_long),
        ("override_redirect", c.c_int), ("screen", c.c_void_p),
    ]


class XImage(c.Structure):
    _fields_ = [
        ("width", c.c_int), ("height", c.c_int), ("xoffset", c.c_int),
        ("format", c.c_int), ("data", c.c_void_p), ("byte_order", c.c_int),
        ("bitmap_unit", c.c_int), ("bitmap_bit_order", c.c_int),
        ("bitmap_pad", c.c_int), ("depth", c.c_int), ("bytes_per_line", c.c_int),
        ("bits_per_pixel", c.c_int), ("red_mask", c.c_ulong),
        ("green_mask", c.c_ulong), ("blue_mask", c.c_ulong),
        ("obdata", c.c_void_p), ("functions", c.c_void_p * 6),
    ]


class ClientMessage(c.Structure):
    _fields_ = [("type", c.c_int), ("serial", c.c_ulong), ("send_event", c.c_int),
        ("display", c.c_void_p), ("window", c.c_ulong), ("message_type", c.c_ulong),
        ("format", c.c_int), ("data", c.c_long * 5)]


class XEvent(c.Union):
    _fields_ = [("client", ClientMessage), ("padding", c.c_long * 24)]


class X11:
    def __init__(self):
        self.lib = c.CDLL(ctypes.util.find_library("X11"))
        self.xtest = c.CDLL(ctypes.util.find_library("Xtst"))
        definitions = {
            "XOpenDisplay": (c.c_void_p, [c.c_char_p]),
            "XCloseDisplay": (c.c_int, [c.c_void_p]),
            "XDefaultRootWindow": (c.c_ulong, [c.c_void_p]),
            "XInternAtom": (c.c_ulong, [c.c_void_p, c.c_char_p, c.c_int]),
            "XGetWindowProperty": (c.c_int, [c.c_void_p, c.c_ulong, c.c_ulong,
                c.c_long, c.c_long, c.c_int, c.c_ulong, c.POINTER(c.c_ulong),
                c.POINTER(c.c_int), c.POINTER(c.c_ulong), c.POINTER(c.c_ulong),
                c.POINTER(c.POINTER(c.c_ubyte))]),
            "XFree": (c.c_int, [c.c_void_p]),
            "XGetWindowAttributes": (c.c_int, [c.c_void_p, c.c_ulong, c.POINTER(WindowAttributes)]),
            "XGetInputFocus": (c.c_int, [c.c_void_p, c.POINTER(c.c_ulong), c.POINTER(c.c_int)]),
            "XQueryPointer": (c.c_int, [c.c_void_p, c.c_ulong, c.POINTER(c.c_ulong),
                c.POINTER(c.c_ulong), c.POINTER(c.c_int), c.POINTER(c.c_int),
                c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(c.c_uint)]),
            "XQueryTree": (c.c_int, [c.c_void_p, c.c_ulong, c.POINTER(c.c_ulong),
                c.POINTER(c.c_ulong), c.POINTER(c.POINTER(c.c_ulong)), c.POINTER(c.c_uint)]),
            "XTranslateCoordinates": (c.c_int, [c.c_void_p, c.c_ulong, c.c_ulong,
                c.c_int, c.c_int, c.POINTER(c.c_int), c.POINTER(c.c_int), c.POINTER(c.c_ulong)]),
            "XGetImage": (c.POINTER(XImage), [c.c_void_p, c.c_ulong, c.c_int,
                c.c_int, c.c_uint, c.c_uint, c.c_ulong, c.c_int]),
            "XDestroyImage": (c.c_int, [c.POINTER(XImage)]),
            "XSync": (c.c_int, [c.c_void_p, c.c_int]),
            "XSendEvent": (c.c_int, [c.c_void_p, c.c_ulong, c.c_int, c.c_long, c.POINTER(XEvent)]),
        }
        for name, (result, arguments) in definitions.items():
            function = getattr(self.lib, name)
            function.restype, function.argtypes = result, arguments
        self.xtest.XTestFakeMotionEvent.argtypes = [c.c_void_p, c.c_int, c.c_int, c.c_int, c.c_ulong]
        self.xtest.XTestFakeButtonEvent.argtypes = [c.c_void_p, c.c_uint, c.c_int, c.c_ulong]
        self.lib.XSetErrorHandler.restype = c.c_void_p
        self.lib.XSetErrorHandler.argtypes = [c.c_void_p]
        self.error_handler = c.CFUNCTYPE(c.c_int, c.c_void_p, c.c_void_p)(lambda _display, _error: 0)
        self.previous_error_handler = self.lib.XSetErrorHandler(c.cast(self.error_handler, c.c_void_p))
        self.display = self.lib.XOpenDisplay(None)
        if not self.display:
            raise RuntimeError("Cannot connect to the current X11 session")
        self.root = self.lib.XDefaultRootWindow(self.display)
        self.owned = {}

    def property(self, window, name):
        actual, format_, count, remaining = c.c_ulong(), c.c_int(), c.c_ulong(), c.c_ulong()
        data = c.POINTER(c.c_ubyte)()
        atom = self.lib.XInternAtom(self.display, name.encode(), 0)
        status = self.lib.XGetWindowProperty(self.display, window, atom, 0, 4096, 0, 0,
            c.byref(actual), c.byref(format_), c.byref(count), c.byref(remaining), c.byref(data))
        if status or not data:
            return [] if format_.value == 32 else b""
        try:
            if format_.value == 32:
                values = c.cast(data, c.POINTER(c.c_ulong))
                return [values[index] for index in range(count.value)]
            return c.string_at(data, count.value * max(format_.value // 8, 1))
        finally:
            self.lib.XFree(data)

    def find(self, pid, title, include_hidden=False):
        clients = self.property(self.root, "_NET_CLIENT_LIST")
        if include_hidden:
            candidates, level = list(clients), [self.root]
            for _ in range(3):
                following = []
                for parent_window in level:
                    root, parent, children, count = c.c_ulong(), c.c_ulong(), c.POINTER(c.c_ulong)(), c.c_uint()
                    if self.lib.XQueryTree(self.display, parent_window, c.byref(root), c.byref(parent),
                            c.byref(children), c.byref(count)) and children:
                        following.extend(children[index] for index in range(count.value))
                        self.lib.XFree(children)
                candidates.extend(following)
                level = following
            clients = dict.fromkeys(candidates)
        for window in clients:
            if self.property(window, "_NET_WM_PID") != [pid]:
                continue
            # Other applications' titles and pixels are never read.
            name = self.property(window, "_NET_WM_NAME")
            if not name:
                name = self.property(window, "WM_NAME")
            if name.decode("utf-8", "replace") == title:
                self.owned[window] = pid
                return window
        return None

    def attributes(self, window):
        if window not in self.owned:
            raise RuntimeError("Refusing an operation on an unowned window")
        if self.property(window, "_NET_WM_PID") != [self.owned[window]]:
            raise RuntimeError("Owned window PID changed")
        attributes = WindowAttributes()
        if not self.lib.XGetWindowAttributes(self.display, window, c.byref(attributes)):
            raise RuntimeError("Owned window no longer exists")
        return attributes

    def close_request(self, window):
        self.attributes(window)
        delete = self.lib.XInternAtom(self.display, b"WM_DELETE_WINDOW", 0)
        if delete not in self.property(window, "WM_PROTOCOLS"):
            raise RuntimeError("Owned window does not accept WM_DELETE_WINDOW")
        event = XEvent()
        event.client = ClientMessage(33, 0, 1, self.display, window,
            self.lib.XInternAtom(self.display, b"WM_PROTOCOLS", 0), 32,
            (c.c_long * 5)(delete, 0, 0, 0, 0))
        if not self.lib.XSendEvent(self.display, window, 0, 0, c.byref(event)):
            raise RuntimeError("Owned window close request was rejected")
        self.lib.XSync(self.display, 0)

    def visible(self, window):
        return self.attributes(window).map_state == 2

    def focused(self, window):
        self.attributes(window)
        current, revert = c.c_ulong(), c.c_int()
        self.lib.XGetInputFocus(self.display, c.byref(current), c.byref(revert))
        for _ in range(32):
            if current.value == window:
                return True
            if current.value in (0, 1, self.root):
                return False
            root, parent, children, count = c.c_ulong(), c.c_ulong(), c.POINTER(c.c_ulong)(), c.c_uint()
            if not self.lib.XQueryTree(self.display, current, c.byref(root), c.byref(parent),
                    c.byref(children), c.byref(count)):
                return False
            if children:
                self.lib.XFree(children)
            current = parent
        return False

    def click(self, window, logical_x, logical_y):
        attributes = self.attributes(window)
        if attributes.map_state != 2:
            raise RuntimeError("Refusing a click on an invisible window")
        x = round(logical_x * attributes.width / 400)
        y = round(logical_y * attributes.width / 400)
        if not 0 <= x < attributes.width or not 0 <= y < attributes.height:
            raise RuntimeError("Click is outside the owned window")
        root_x, root_y, child = c.c_int(), c.c_int(), c.c_ulong()
        if not self.lib.XTranslateCoordinates(self.display, window, self.root, x, y,
                c.byref(root_x), c.byref(root_y), c.byref(child)):
            raise RuntimeError("Could not locate the owned click target")
        target = self.root
        for _ in range(32):
            if target == window:
                break
            relative_x, relative_y = c.c_int(), c.c_int()
            if not self.lib.XTranslateCoordinates(self.display, self.root, target, root_x, root_y,
                    c.byref(relative_x), c.byref(relative_y), c.byref(child)) or not child.value:
                raise RuntimeError("Owned click target is covered; no input was sent")
            target = child.value
        else:
            raise RuntimeError("Could not prove ownership of the click target")
        if not self.xtest.XTestFakeMotionEvent(self.display, -1, root_x, root_y, 0):
            raise RuntimeError("XTest rejected own-window pointer motion")
        self.lib.XSync(self.display, 0)
        time.sleep(0.10)
        pointer_root, pointer_child = c.c_ulong(), c.c_ulong()
        pointer_root_x, pointer_root_y, pointer_x, pointer_y = c.c_int(), c.c_int(), c.c_int(), c.c_int()
        pointer_mask = c.c_uint()
        self.lib.XQueryPointer(self.display, window, c.byref(pointer_root), c.byref(pointer_child),
            c.byref(pointer_root_x), c.byref(pointer_root_y), c.byref(pointer_x), c.byref(pointer_y),
            c.byref(pointer_mask))
        print(json.dumps({"click": {"window": window, "expected_client": [x, y],
            "actual_client": [pointer_x.value, pointer_y.value], "mask_after_motion": pointer_mask.value}}), flush=True)
        if abs(pointer_x.value - x) > 1 or abs(pointer_y.value - y) > 1:
            raise RuntimeError("XTest motion did not reach the owned target; no button input was sent")
        if not self.xtest.XTestFakeButtonEvent(self.display, 1, 1, 0):
            raise RuntimeError("XTest rejected own-window button press")
        self.lib.XSync(self.display, 0)
        time.sleep(0.08)
        if not self.xtest.XTestFakeButtonEvent(self.display, 1, 0, 0):
            raise RuntimeError("XTest rejected own-window button release")
        self.lib.XSync(self.display, 0)
        time.sleep(0.10)

    def screenshot(self, window, path):
        attributes = self.attributes(window)
        if attributes.map_state != 2:
            raise RuntimeError("Refusing a screenshot of an invisible window")
        image = self.lib.XGetImage(self.display, window, 0, 0, attributes.width,
            attributes.height, c.c_ulong(-1).value, 2)
        if not image:
            raise RuntimeError("Could not capture the owned window")
        try:
            value = image.contents
            if value.bits_per_pixel != 32 or value.red_mask != 0xff0000 or value.byte_order != 0:
                raise RuntimeError("Screenshot decoder expects the current session's 32-bit BGRX visual")
            raw = c.string_at(value.data, value.bytes_per_line * value.height)
            Image.frombytes("RGB", (value.width, value.height), raw, "raw", "BGRX",
                value.bytes_per_line, 1).save(path)
        finally:
            self.lib.XDestroyImage(image)

    def close(self):
        if self.display:
            self.lib.XCloseDisplay(self.display)
            self.display = None
            self.lib.XSetErrorHandler(self.previous_error_handler)


def wait_until(predicate, description, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.05)
    raise RuntimeError(f"Timed out: {description}")


def desktop_quote(value):
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"').replace("$", "\\$").replace("`", "\\`").replace("%", "%%") + '"'


def fixture(output):
    # Unix socket pathnames are short; a deeply nested artifact output path can exceed their limit.
    root = Path(tempfile.mkdtemp(prefix="brows-gui-"))
    paths = {name: root / name for name in ["home", "config", "config-dirs", "data", "data-dirs", "runtime", "cache", "bin", "chrome-data"]}
    for path in paths.values():
        path.mkdir(mode=0o700)
    applications = paths["data"] / "applications"
    applications.mkdir()
    executable = paths["bin"] / "vivaldi-stable"
    executable.write_text(f"#!{sys.executable}\n" +
        "import fcntl, json, os, sys\n"
        "with open(os.environ['BROWS_GUI_CAPTURE_LOG'], 'a') as capture:\n"
        " fcntl.flock(capture, fcntl.LOCK_EX)\n"
        " capture.write(json.dumps(sys.argv[1:], ensure_ascii=False) + '\\n')\n"
        " capture.flush()\n", encoding="utf-8")
    executable.chmod(0o700)
    desktop = applications / "vivaldi-stable.desktop"
    desktop.write_text("[Desktop Entry]\nType=Application\nName=検証用 Vivaldi\n"
        f"Exec={desktop_quote(str(executable))} --fixture-fixed \"two words\" %u\n"
        "Terminal=false\nStartupNotify=false\nDBusActivatable=false\n"
        "MimeType=x-scheme-handler/http;x-scheme-handler/https;\n", encoding="utf-8")
    profiles = paths["config"] / "vivaldi"
    for name in ["Default", "Profile 1"]:
        (profiles / name).mkdir(parents=True)
        (profiles / name / "Preferences").write_text("{}", encoding="utf-8")
    (profiles / "Local State").write_text(json.dumps({"profile": {"info_cache": {
        "Default": {"name": "個人"}, "Profile 1": {"name": "仕事用"}}}}, ensure_ascii=False), encoding="utf-8")
    env = os.environ.copy()
    # Only child processes receive an isolated HOME; the session's authentication stays inherited.
    if not env.get("XAUTHORITY") and env.get("HOME"):
        env["XAUTHORITY"] = str(Path(env["HOME"]) / ".Xauthority")
    env.update({
        "HOME": str(paths["home"]), "XDG_CONFIG_HOME": str(paths["config"]),
        "XDG_CONFIG_DIRS": str(paths["config-dirs"]), "XDG_DATA_HOME": str(paths["data"]),
        "XDG_DATA_DIRS": str(paths["data-dirs"]), "XDG_RUNTIME_DIR": str(paths["runtime"]),
        "XDG_CACHE_HOME": str(paths["cache"]), "CHROME_CONFIG_HOME": str(paths["config"]),
        "CHROME_USER_DATA_DIR": str(paths["chrome-data"]), "LC_ALL": "ja_JP.UTF-8",
        "LANG": "ja_JP.UTF-8", "BROWS_GUI_CAPTURE_LOG": str(root / "capture.jsonl"),
    })
    for name in ["EFRAME_SCREENSHOT_TO", "BROWS_TEST_CONFIG", "BROWS_TEST_WORKER"]:
        env.pop(name, None)
    subprocess.run(["desktop-file-validate", str(desktop)], env=env, check=True)
    subprocess.run(["update-desktop-database", str(applications)], env=env, check=True)
    return root, env


def captured(root):
    path = root / "capture.jsonl"
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()] if path.exists() else []


def write_manual_wrapper(output, binary, env):
    variables = ["HOME", "XDG_CONFIG_HOME", "XDG_CONFIG_DIRS", "XDG_DATA_HOME",
        "XDG_DATA_DIRS", "XDG_RUNTIME_DIR", "XDG_CACHE_HOME", "CHROME_CONFIG_HOME",
        "CHROME_USER_DATA_DIR", "LC_ALL", "LANG", "BROWS_GUI_CAPTURE_LOG"]
    wrapper = output / "run-brows.sh"
    lines = ["#!/bin/sh", "set -eu", "",
        '# Preserve session authentication before replacing only the child HOME.',
        'if [ -z "${XAUTHORITY:-}" ] && [ -n "${HOME:-}" ]; then',
        '    export XAUTHORITY="$HOME/.Xauthority"', "fi", ""]
    lines.extend(f"export {name}={shlex.quote(env[name])}" for name in variables)
    lines.extend(["unset EFRAME_SCREENSHOT_TO BROWS_TEST_CONFIG BROWS_TEST_WORKER",
        f'exec {shlex.quote(str(binary))} "$@"', ""])
    wrapper.write_text("\n".join(lines), encoding="utf-8")
    wrapper.chmod(0o700)
    return wrapper


def verify(args):
    output = Path(args.output).resolve()
    output.mkdir(parents=True, exist_ok=True)
    root, env = fixture(output)
    report = {"fixture": str(root), "session": os.environ.get("XDG_SESSION_TYPE"), "checks": [], "screenshots": []}
    report["mode"] = "lifecycle-only" if args.lifecycle_only else "selection-and-lifecycle"
    if args.lifecycle_only:
        report["unverified"] = ["GUI browser/profile selection", "FIFO selected launch order",
            "Cancel button input; WM_DELETE_WINDOW exercises the same dismiss path instead"]
    report_path = output / "report.json"
    binary = Path(args.binary).resolve()
    if args.prepare_only:
        report["manual_wrapper"] = str(write_manual_wrapper(output, binary, env))
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps({"fixture": str(root), "report": str(report_path),
            "manual_wrapper": report["manual_wrapper"]}, ensure_ascii=False))
        return
    if not binary.is_file():
        raise RuntimeError("Build the brows executable before GUI verification")
    report["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
    x11 = X11()
    processes = []
    log = (output / "application.log").open("w", encoding="utf-8")

    def check(name, value, **details):
        report["checks"].append({"name": name, "passed": bool(value), **details})
        print(json.dumps(report["checks"][-1], ensure_ascii=False), flush=True)

    def snapshot(window, name):
        path = output / f"{name}.png"
        x11.screenshot(window, path)
        report["screenshots"].append(str(path))

    def secondary(*arguments):
        subprocess.run([str(binary), *arguments], env=env, stdout=log, stderr=log, timeout=35, check=True)

    def select(window, profile=0):
        x11.click(window, 200, args.row_y)
        time.sleep(0.18)
        x11.click(window, 160, args.profile_y + profile * args.profile_step)

    def find_live(process, title, include_hidden=False):
        if process.poll() is not None:
            raise RuntimeError(f"brows exited with {process.returncode} before {title}; inspect application.log")
        return x11.find(process.pid, title, include_hidden)

    urls = [f"https://example.invalid/gui/{index}" for index in range(7)]
    try:
        resident = subprocess.Popen([str(binary), urls[0]], env=env, stdout=log, stderr=log)
        processes.append(resident)
        window = wait_until(lambda: find_live(resident, "brows"), "first picker window", 20)
        wait_until(lambda: x11.visible(window), "first picker visible")
        time.sleep(0.4)
        check("first_visible", True, window=window, pid=resident.pid)
        check("first_focus", wait_focus(x11, window))
        snapshot(window, "01-picker-japanese")
        if args.lifecycle_only:
            x11.close_request(window)
        else:
            x11.click(window, 200, args.row_y)
            time.sleep(0.2)
            snapshot(window, "02-profiles-japanese")
            x11.click(window, 160, args.profile_y)
            wait_until(lambda: len(captured(root)) == 1, "first synthetic launch")
        wait_until(lambda: not x11.visible(window), "picker hidden after dismissal")
        check("hidden_retained", resident.poll() is None, window=window, pid=resident.pid)

        secondary(urls[1])
        wait_until(lambda: x11.visible(window), "retained picker visible again")
        time.sleep(0.2)
        check("same_window_resurfaced", x11.find(resident.pid, "brows") == window, window=window)
        check("resurface_focus", wait_focus(x11, window))
        snapshot(window, "03-retained-resurface")
        if args.lifecycle_only:
            x11.close_request(window)
        else:
            select(window, profile=1)
            wait_until(lambda: len(captured(root)) == 2, "second profile launch")
        wait_until(lambda: not x11.visible(window), "picker hidden again")

        if args.lifecycle_only:
            check("close_request_did_not_launch_browser", not captured(root))
        else:
            secondary(urls[2])
            wait_until(lambda: x11.visible(window), "burst first URL visible")
            for url in urls[3:]:
                secondary(url)
            check("burst_waits_for_selection", len(captured(root)) == 2)
            snapshot(window, "04-burst-before-selection")
            for count in range(3, 8):
                select(window)
                wait_until(lambda: len(captured(root)) == count, f"FIFO selection {count - 2}")
                if count < 7:
                    wait_until(lambda: x11.visible(window), "next FIFO URL")
                    time.sleep(0.12)
            wait_until(lambda: not x11.visible(window), "picker hidden after FIFO drain")
            records = captured(root)
            expected = [["--fixture-fixed", "two words", url, f"--profile-directory={'Profile 1' if index == 1 else 'Default'}"]
                for index, url in enumerate(urls)]
            check("fifo_fixed_args_profiles", records == expected, expected=expected, actual=records)
            check("same_retained_window_after_burst", resident.poll() is None, window=window)

        settings = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=log)
        processes.append(settings)
        settings_window = wait_until(lambda: find_live(settings, "brows - 設定"), "Japanese settings window", 20)
        wait_until(lambda: x11.visible(settings_window), "settings visible")
        time.sleep(0.4)
        snapshot(settings_window, "05-settings-japanese")
        check("settings_japanese", True, window=settings_window, pid=settings.pid)
        check("no_default_or_autostart_change", not (root / "config/mimeapps.list").exists()
            and not (root / "data/applications/mimeapps.list").exists()
            and not (root / "config/autostart").exists())

        settings.terminate()
        settings.wait(timeout=3)
        secondary("--quit")
        resident.wait(timeout=3)
        auto_root, env = fixture(output)
        report["auto_rule_fixture"] = str(auto_root)
        config_directory = auto_root / "config/brows"
        config_directory.mkdir()
        (config_directory / "config.toml").write_text(
            '[[rules]]\npattern = "auto.invalid"\nbrowser = "vivaldi-stable.desktop#Default"\n', encoding="utf-8")
        auto_url = "https://auto.invalid/initial-rule"
        auto = subprocess.Popen([str(binary), auto_url], env=env, stdout=log, stderr=log)
        processes.append(auto)
        wait_until(lambda: len(captured(auto_root)) == 1, "initial automatic synthetic launch", 20)
        auto_window = wait_until(lambda: find_live(auto, "brows", True), "hidden resident after initial rule", 20)
        wait_until(lambda: not x11.visible(auto_window), "initial automatic rule keeps picker hidden")
        check("initial_auto_rule_retains_resident", auto.poll() is None, window=auto_window, pid=auto.pid)
        check("initial_auto_rule_args", captured(auto_root) == [["--fixture-fixed", "two words", auto_url, "--profile-directory=Default"]])
        secondary("https://unmatched.invalid/after-rule")
        wait_until(lambda: x11.visible(auto_window), "unmatched URL resurfaces automatic resident")
        check("initial_auto_rule_same_window_resurfaces", x11.find(auto.pid, "brows") == auto_window, window=auto_window)
        check("initial_auto_rule_resurface_focus", wait_focus(x11, auto_window))
        snapshot(auto_window, "06-auto-rule-resurface")
        if args.lifecycle_only:
            x11.close_request(auto_window)
        else:
            x11.click(auto_window, 42, args.cancel_y)
        wait_until(lambda: not x11.visible(auto_window), "cancel hides the retained picker")
        check("close_request_hides_retains_same_window" if args.lifecycle_only else "cancel_hides_retains_same_window",
            auto.poll() is None and len(captured(auto_root)) == 1, window=auto_window)
        check("auto_rule_no_default_or_autostart_change", not (auto_root / "config/mimeapps.list").exists()
            and not (auto_root / "data/applications/mimeapps.list").exists()
            and not (auto_root / "config/autostart").exists())
    except Exception as error:
        report["error"] = str(error)
        raise
    finally:
        try:
            secondary("--quit")
        except (OSError, subprocess.SubprocessError):
            pass
        for process in processes:
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=3)
        log.close()
        x11.close()
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps({"report": str(report_path)}, ensure_ascii=False), flush=True)
    if not all(item["passed"] for item in report["checks"]):
        raise RuntimeError("One or more GUI checks failed; inspect report.json and owned-window screenshots")


def wait_focus(x11, window):
    try:
        return bool(wait_until(lambda: x11.focused(window), "focus request", timeout=1.5))
    except RuntimeError:
        return False


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default="target/debug/brows")
    parser.add_argument("--output", required=True)
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--lifecycle-only", action="store_true",
        help="Use owned-window WM_DELETE_WINDOW; leave GUI selection and FIFO launch order unverified")
    parser.add_argument("--row-y", type=float, default=89)
    parser.add_argument("--profile-y", type=float, default=126)
    parser.add_argument("--profile-step", type=float, default=37)
    parser.add_argument("--cancel-y", type=float, default=140)
    arguments = parser.parse_args()
    try:
        verify(arguments)
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
