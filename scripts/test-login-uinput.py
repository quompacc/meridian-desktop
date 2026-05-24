#!/usr/bin/env python3
"""Exercise meridian-login with a virtual keyboard.

This script is intentionally system-facing: run it as root on the login host.
It can prepare a disposable user, restart meridian-login, type credentials via
/dev/uinput, verify the compositor handover in journald, and clean up again.
"""

from __future__ import annotations

import argparse
import fcntl
import os
import pwd
import re
import struct
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Iterable


EV_SYN = 0
EV_KEY = 1
SYN_REPORT = 0
UI_SET_EVBIT = 0x40045564
UI_SET_KEYBIT = 0x40045565
UI_DEV_CREATE = 0x5501
UI_DEV_DESTROY = 0x5502

KEY_TAB = 15
KEY_ENTER = 28
KEYS = {
    "1": 2,
    "2": 3,
    "3": 4,
    "4": 5,
    "5": 6,
    "6": 7,
    "7": 8,
    "8": 9,
    "9": 10,
    "0": 11,
    "-": 12,
    "q": 16,
    "w": 17,
    "e": 18,
    "r": 19,
    "t": 20,
    "y": 21,
    "u": 22,
    "i": 23,
    "o": 24,
    "p": 25,
    "a": 30,
    "s": 31,
    "d": 32,
    "f": 33,
    "g": 34,
    "h": 35,
    "j": 36,
    "k": 37,
    "l": 38,
    "z": 44,
    "x": 45,
    "c": 46,
    "v": 47,
    "b": 48,
    "n": 49,
    "m": 50,
}
SUPPORTED_TEXT = re.compile(r"^[a-z0-9-]+$")


@dataclass
class CmdResult:
    args: list[str]
    returncode: int
    stdout: str
    stderr: str


class TestFailure(RuntimeError):
    pass


def log(message: str) -> None:
    print(f"[login-uinput] {message}", flush=True)


def run_cmd(
    args: Iterable[str],
    *,
    check: bool = True,
    input_text: str | None = None,
) -> CmdResult:
    argv = list(args)
    proc = subprocess.run(
        argv,
        input=input_text,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    result = CmdResult(argv, proc.returncode, proc.stdout, proc.stderr)
    if check and proc.returncode != 0:
        detail = (proc.stderr or proc.stdout).strip()
        raise TestFailure(f"{' '.join(argv)} failed with {proc.returncode}: {detail}")
    return result


def require_root() -> None:
    if os.geteuid() != 0:
        raise TestFailure("run as root, for example: sudo scripts/test-login-uinput.py")


def validate_text(label: str, value: str) -> None:
    if not SUPPORTED_TEXT.fullmatch(value):
        raise TestFailure(f"{label} must contain only lowercase letters, digits, or '-'")


def user_exists(username: str) -> bool:
    try:
        pwd.getpwnam(username)
        return True
    except KeyError:
        return False


def prepare_user(username: str, password: str) -> None:
    if not user_exists(username):
        log(f"creating user {username}")
        run_cmd(["useradd", "-m", "-s", "/bin/bash", "-G", "video,render,input", username])
    else:
        log(f"user {username} already exists")
        run_cmd(["usermod", "-aG", "video,render,input", username])

    log(f"setting test password for {username}")
    run_cmd(["chpasswd"], input_text=f"{username}:{password}\n")
    run_cmd(["usermod", "-U", username])


def lock_user(username: str) -> None:
    if not user_exists(username):
        log(f"user {username} does not exist; lock skipped")
        return
    log(f"locking {username}")
    run_cmd(["usermod", "-L", username], check=False)


def terminate_user(username: str) -> None:
    if user_exists(username):
        log(f"terminating user session for {username}")
        run_cmd(["loginctl", "terminate-user", username], check=False)


class VirtualKeyboard:
    def __init__(self) -> None:
        self._file = None

    def __enter__(self) -> "VirtualKeyboard":
        self._file = Path("/dev/uinput").open("wb", buffering=0)
        fcntl.ioctl(self._file, UI_SET_EVBIT, EV_KEY)
        fcntl.ioctl(self._file, UI_SET_EVBIT, EV_SYN)
        for code in sorted(set(KEYS.values()) | {KEY_TAB, KEY_ENTER}):
            fcntl.ioctl(self._file, UI_SET_KEYBIT, code)

        name = b"meridian-login-smoke-keyboard"
        user_dev = struct.pack("80sHHHHi", name, 3, 0x1234, 0x5678, 1, 0)
        self._file.write(user_dev + bytes(1116 - len(user_dev)))
        fcntl.ioctl(self._file, UI_DEV_CREATE)
        time.sleep(0.4)
        return self

    def __exit__(self, _exc_type, _exc, _tb) -> None:
        if self._file is None:
            return
        try:
            fcntl.ioctl(self._file, UI_DEV_DESTROY)
        finally:
            self._file.close()
            self._file = None

    def emit(self, ev_type: int, code: int, value: int) -> None:
        if self._file is None:
            raise TestFailure("virtual keyboard is not open")
        self._file.write(struct.pack("llHHi", 0, 0, ev_type, code, value))

    def tap(self, code: int) -> None:
        self.emit(EV_KEY, code, 1)
        self.emit(EV_SYN, SYN_REPORT, 0)
        time.sleep(0.025)
        self.emit(EV_KEY, code, 0)
        self.emit(EV_SYN, SYN_REPORT, 0)
        time.sleep(0.045)

    def type_text(self, text: str) -> None:
        for char in text:
            self.tap(KEYS[char])


def current_journal_time() -> str:
    return datetime.now().strftime("%Y-%m-%d %H:%M:%S")


def restart_login() -> None:
    log("restarting meridian-login.service")
    run_cmd(["systemctl", "restart", "meridian-login.service"])


def journal_since(since: str) -> str:
    result = run_cmd(
        [
            "journalctl",
            "-b",
            "--since",
            since,
            "--no-pager",
            "-o",
            "short-iso",
        ]
    )
    return result.stdout


def pgrep_user(username: str, process: str) -> bool:
    result = run_cmd(["pgrep", "-u", username, "-x", process], check=False)
    return result.returncode == 0


def verify_login(username: str, since: str) -> None:
    log("checking spawned compositor processes")
    process_deadline = time.monotonic() + 6.0
    while True:
        missing = [name for name in ("meridian", "meridian-shell") if not pgrep_user(username, name)]
        if not missing:
            break
        if time.monotonic() >= process_deadline:
            processes = run_cmd(["pgrep", "-a", "-u", username], check=False).stdout.strip()
            raise TestFailure(f"missing processes for {username}: {', '.join(missing)}\n{processes}")
        time.sleep(0.4)

    log("checking meridian-login journal markers")
    required = ("auth ok", "compositor spawned", "ipc handover", "ipc exit")
    journal_deadline = time.monotonic() + 10.0
    while True:
        journal = journal_since(since)
        missing_markers = [marker for marker in required if marker not in journal]
        if not missing_markers:
            break
        if time.monotonic() >= journal_deadline:
            raise TestFailure(f"missing journal markers: {', '.join(missing_markers)}\n{journal}")
        time.sleep(0.5)

    forbidden = ("panic", "fatal drm startup failure", "cursor theme miss", "cursor theme fallback")
    hits = [marker for marker in forbidden if marker in journal.lower()]
    if hits:
        raise TestFailure(f"forbidden journal markers found: {', '.join(hits)}\n{journal}")


def run_login_test(args: argparse.Namespace) -> None:
    since = current_journal_time()
    with VirtualKeyboard() as keyboard:
        if args.restart_login:
            restart_login()
            time.sleep(args.login_ready_delay)

        log(f"typing credentials for {args.username}")
        keyboard.type_text(args.username)
        keyboard.tap(KEY_TAB)
        keyboard.type_text(args.password)
        keyboard.tap(KEY_ENTER)
        time.sleep(args.verify_delay)

    verify_login(args.username, since)
    log("login smoke test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--username", default="fakeuser")
    parser.add_argument("--password", default="pass1234")
    parser.add_argument("--prepare-user", action="store_true")
    parser.add_argument("--run", action="store_true", help="run the virtual-keyboard login test")
    parser.add_argument("--lock-user", action="store_true", help="lock the user after cleanup")
    parser.add_argument("--keep-session", action="store_true", help="leave the compositor session running")
    parser.add_argument("--no-restart-login", dest="restart_login", action="store_false")
    parser.add_argument("--login-ready-delay", type=float, default=3.2)
    parser.add_argument("--verify-delay", type=float, default=5.0)
    parser.set_defaults(restart_login=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        require_root()
        validate_text("username", args.username)
        validate_text("password", args.password)

        if args.prepare_user:
            prepare_user(args.username, args.password)
        elif not user_exists(args.username):
            raise TestFailure(f"user {args.username} does not exist; use --prepare-user")

        try:
            if args.run:
                terminate_user(args.username)
                run_login_test(args)
            else:
                log("nothing to run; pass --run")
        finally:
            if not args.keep_session:
                restart_login()
                terminate_user(args.username)
            if args.lock_user:
                lock_user(args.username)
        return 0
    except TestFailure as exc:
        print(f"[login-uinput] ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
