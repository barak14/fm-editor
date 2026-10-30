#!/usr/bin/env python3
"""Check a release GUI survives startup without starting the debug MCP server.

Use an isolated graphical session on Linux (for example xvfb-run). No FM process
or save is required; this does not verify game attachment or editing.
"""

import argparse
from pathlib import Path
import socket
import subprocess
import tempfile
import time


MCP_PORT = 7799


def mcp_port_open():
    try:
        with socket.create_connection(("127.0.0.1", MCP_PORT), timeout=0.2):
            return True
    except OSError:
        return False


def smoke(command, seconds=3.0):
    if seconds <= 0:
        raise ValueError("Startup observation time must be positive")
    if mcp_port_open():
        raise RuntimeError("Port 7799 is already in use; run the smoke test in a clean session")
    with tempfile.TemporaryFile(mode="w+b") as log:
        process = subprocess.Popen(
            command, stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT
        )
        try:
            time.sleep(seconds)
            status = process.poll()
            if status is not None:
                log.seek(0)
                detail = log.read().decode("utf-8", errors="replace")
                raise RuntimeError(f"Editor exited during startup (code {status}):\n{detail}")
            if mcp_port_open():
                raise RuntimeError("Release startup opened the debug MCP port 7799")
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    args = parser.parse_args(argv)
    if not args.binary.is_file():
        parser.error(f"Release executable not found: {args.binary}")
    try:
        smoke([str(args.binary.resolve())])
    except (OSError, RuntimeError, ValueError) as error:
        parser.exit(1, f"Startup smoke test failed: {error}\n")
    print("Release survived startup without the debug MCP listener; game workflows remain unverified.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
