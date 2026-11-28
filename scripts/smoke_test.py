#!/usr/bin/env python3
"""
Comprehensive Smoke Test Suite for LavaTerm.
Verifies real binary execution in:
  1. Headless simulation mode (--headless --frames 30)
  2. Headless audio reactive mode (--headless --frames 30 --audio)
  3. Headless combined mode (--headless --frames 30 --system --audio)
  4. Snapshot rendering mode (--snapshot --width 60 --height 20)
  5. Audio capture device enumeration (--list-audio-devices)
  6. Interactive full-screen TUI in real PTY with clean exit on 'q'
  7. Interactive TUI graceful shutdown on SIGINT (Ctrl+C)
  8. Interactive TUI graceful shutdown on SIGTERM
  9. Interactive TUI with native audio capture in PTY
 10. Packaged binaries verification (AppImage, deb, arch pkg)
"""

import argparse
import fcntl
import os
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import termios
import time

def detect_cargo_version() -> str:
    cargo_toml = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "Cargo.toml")
    with open(cargo_toml, "r", encoding="utf-8") as fh:
        for line in fh:
            match = re.match(r'^version\s*=\s*"([^"]+)"', line.strip())
            if match:
                return match.group(1)
    raise RuntimeError(f"Could not determine version from {cargo_toml}")

BINARY_PATH = "./target/release/lavaterm"
REQUIRE_PACKAGES = False
CARGO_VERSION = detect_cargo_version()
PACKAGE_VERSION = CARGO_VERSION

def log_test(name: str):
    print(f"\n========================================================")
    print(f"RUNNING SMOKE TEST: {name}")
    print(f"========================================================")

def run_command_checked(args, desc: str, timeout=10):
    log_test(desc)
    print(f"Command: {' '.join(args)}")
    proc = subprocess.run(args, capture_output=True, text=True, timeout=timeout)
    print(f"Exit code: {proc.returncode}")
    if proc.stdout:
        print("--- stdout ---")
        print(proc.stdout.strip())
    if proc.stderr:
        print("--- stderr ---")
        print(proc.stderr.strip())
    assert proc.returncode == 0, f"{desc} failed with exit code {proc.returncode}"
    return proc

def test_headless():
    proc = run_command_checked(
        [BINARY_PATH, "--headless", "--frames", "30"],
        "Headless Simulation (30 frames)"
    )
    assert "Starting LavaTerm headless simulation" in proc.stdout
    assert "Headless simulation completed successfully." in proc.stdout

def test_headless_audio():
    proc = run_command_checked(
        [BINARY_PATH, "--headless", "--frames", "30", "--audio"],
        "Headless Audio Reactive Simulation"
    )
    assert "audio=true" in proc.stdout
    assert "Headless simulation completed successfully." in proc.stdout

def test_headless_system_audio():
    log_test("Headless System + Audio Conflict Rejection")
    args = [BINARY_PATH, "--headless", "--frames", "30", "--system", "--audio"]
    print(f"Command: {' '.join(args)}")
    proc = subprocess.run(args, capture_output=True, text=True, timeout=10)
    print(f"Exit code: {proc.returncode}")
    if proc.stdout:
        print("--- stdout ---")
        print(proc.stdout.strip())
    if proc.stderr:
        print("--- stderr ---")
        print(proc.stderr.strip())
    assert proc.returncode != 0, "Expected non-zero exit code when combining --system and --audio"
    assert "conflict" in proc.stderr.lower(), f"Expected conflict message in stderr, got: {proc.stderr}"

def test_snapshot():
    proc = run_command_checked(
        [BINARY_PATH, "--snapshot", "--width", "60", "--height", "20"],
        "Snapshot Frame Rendering"
    )
    assert len(proc.stdout) > 100, "Snapshot output is too short"
    assert "\x1b[" in proc.stdout or "▀" in proc.stdout or "█" in proc.stdout

def test_list_audio_devices():
    proc = run_command_checked(
        [BINARY_PATH, "--list-audio-devices"],
        "Audio Capture Device Enumeration"
    )
    assert "Available Audio Capture Devices" in proc.stdout

def run_in_pty(extra_args=None, terminate_via="q", timeout=5.0):
    """
    Spawns binary in a real pseudo-terminal with 80x24 winsize,
    verifies ANSI frame output, sends termination ('q', SIGINT, or SIGTERM),
    and drains output until clean exit code 0.
    """
    args = [BINARY_PATH] + (extra_args or [])
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))

    pid = os.fork()
    if pid == 0:
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
        os.dup2(slave, 0)
        os.dup2(slave, 1)
        os.dup2(slave, 2)
        os.close(slave)
        os.close(master)
        os.execv(args[0], args)
    else:
        os.close(slave)
        fcntl.fcntl(master, fcntl.F_SETFL, os.O_NONBLOCK)

        total_bytes = 0
        startup_frame_detected = False
        start = time.time()

        while time.time() - start < 1.5:
            r, _, _ = select.select([master], [], [], 0.05)
            if master in r:
                try:
                    data = os.read(master, 16384)
                    if data:
                        total_bytes += len(data)
                        if b"\x1b[" in data and total_bytes > 5000:
                            startup_frame_detected = True
                except OSError:
                    break

        print(f"Startup detected: {startup_frame_detected}, bytes read: {total_bytes}")
        assert startup_frame_detected, "Binary did not output ANSI escape frames during startup"
        assert total_bytes >= 10000, f"Expected >= 10,000 bytes of rendered frames, got {total_bytes}"

        if terminate_via == "q":
            print("Sending 'q' keystroke to quit...")
            os.write(master, b"q")
        elif terminate_via == "SIGINT":
            print("Sending SIGINT signal...")
            os.kill(pid, signal.SIGINT)
        elif terminate_via == "SIGTERM":
            print("Sending SIGTERM signal...")
            os.kill(pid, signal.SIGTERM)

        exit_start = time.time()
        clean_exit = False
        exit_code = None
        while time.time() - exit_start < timeout:
            r, _, _ = select.select([master], [], [], 0.05)
            if master in r:
                try:
                    data = os.read(master, 16384)
                    if data:
                        total_bytes += len(data)
                except OSError:
                    pass
            wpid, status = os.waitpid(pid, os.WNOHANG)
            if wpid != 0:
                exit_code = os.waitstatus_to_exitcode(status)
                clean_exit = True
                break

        if not clean_exit:
            print(f"Process {pid} timed out waiting for clean exit!")
            os.kill(pid, signal.SIGKILL)
            os.waitpid(pid, 0)
            os.close(master)
            raise RuntimeError(f"Process did not exit cleanly within {timeout}s")

        os.close(master)
        print(f"Process exited cleanly with status code {exit_code}")
        print(f"Total stream output across test: {total_bytes} bytes")
        assert exit_code == 0, f"Expected exit code 0, got {exit_code}"

def test_interactive_tui():
    log_test("Interactive TUI (PTY, 30 FPS, quit via 'q')")
    run_in_pty(["--fps", "30"], terminate_via="q")

def test_interactive_sigint():
    log_test("Interactive TUI (PTY, graceful shutdown via SIGINT)")
    run_in_pty(["--fps", "30"], terminate_via="SIGINT")

def test_interactive_sigterm():
    log_test("Interactive TUI (PTY, graceful shutdown via SIGTERM)")
    run_in_pty(["--fps", "30"], terminate_via="SIGTERM")

def test_interactive_audio():
    log_test("Interactive TUI with Audio Reactive Stream (PTY, --audio)")
    run_in_pty(["--audio", "--fps", "30"], terminate_via="q")

def test_packaged_artifacts():
    log_test("Packaged Artifacts Verification")
    expected_appimage = f"./dist/ZenLavaTerm-v{PACKAGE_VERSION}-linux-x86_64.AppImage"
    packages = [
        (expected_appimage, "Linux AppImage", True),
        ("./target/deb_test/usr/bin/lavaterm", "Debian Package Binary", True),
        ("./target/arch_test/usr/bin/lavaterm", "Arch Linux Package Binary", False),
    ]

    for bin_path, desc, required_in_pkg_mode in packages:
        if os.path.exists(bin_path):
            print(f"\nVerifying {desc} ({bin_path})...")
            v_proc = subprocess.run([bin_path, "--version"], capture_output=True, text=True)
            assert v_proc.returncode == 0, f"{desc} --version failed"
            assert f"lavaterm {CARGO_VERSION}" in v_proc.stdout, (
                f"Expected 'lavaterm {CARGO_VERSION}', got: {v_proc.stdout}"
            )
            print(f"  ✅ Version check passed: {v_proc.stdout.strip()}")

            h_proc = subprocess.run([bin_path, "--headless", "--frames", "10"], capture_output=True, text=True)
            assert h_proc.returncode == 0, f"{desc} headless failed"
            print("  ✅ Headless simulation passed")

            global BINARY_PATH
            prev = BINARY_PATH
            BINARY_PATH = bin_path
            try:
                run_in_pty(["--fps", "30"], terminate_via="q")
                print(f"  ✅ Interactive PTY launch and clean exit passed")
            finally:
                BINARY_PATH = prev
        elif REQUIRE_PACKAGES and required_in_pkg_mode:
            raise AssertionError(f"Required packaged artifact not found: {bin_path} ({desc})")
        else:
            print(f"Note: {bin_path} not found, skipping {desc}")

def main():
    global BINARY_PATH, REQUIRE_PACKAGES, PACKAGE_VERSION
    parser = argparse.ArgumentParser(description="LavaTerm Linux PTY & Packaging Smoke Test Suite")
    parser.add_argument("binary", nargs="?", default="./target/release/lavaterm", help="Path to lavaterm binary")
    parser.add_argument(
        "--require-packages",
        action="store_true",
        help="Fail instead of skipping when packaged AppImage or extracted DEB binary is missing",
    )
    parser.add_argument(
        "--version",
        dest="package_version",
        default=None,
        help="Override expected packaged artifact version string (defaults to Cargo.toml)",
    )
    parsed = parser.parse_args()
    BINARY_PATH = parsed.binary
    REQUIRE_PACKAGES = parsed.require_packages
    if parsed.package_version:
        PACKAGE_VERSION = parsed.package_version

    print(
        f"Starting LavaTerm Smoke Test Suite against: {BINARY_PATH} "
        f"(binary v{CARGO_VERSION}, package v{PACKAGE_VERSION})"
    )
    assert os.path.exists(BINARY_PATH), f"Target binary not found: {BINARY_PATH}"

    test_headless()
    test_headless_audio()
    test_headless_system_audio()
    test_snapshot()
    test_list_audio_devices()
    test_interactive_tui()
    test_interactive_sigint()
    test_interactive_sigterm()
    test_interactive_audio()
    test_packaged_artifacts()

    print("\n========================================================")
    print("🎉 ALL SMOKE TESTS COMPLETED SUCCESSFULLY!")
    print("========================================================\n")

if __name__ == "__main__":
    main()
