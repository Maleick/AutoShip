#!/usr/bin/env python3
"""Load and activate a binary in Ghidra MCP for offset research.

Two-step process:
  1. /load_program  — imports the binary into Ghidra's project
  2. /open_program  — sets it as the active context so queries work

Use --open-only when the binary is already imported but context is lost
(e.g. after a container restart or when is_current=false in /programs).
"""
import argparse
import json
import subprocess
import sys
import urllib.error
import urllib.request


DEFAULT_BASE_URL = "http://127.0.0.1:8089"
DEFAULT_CONTAINER = "ghidra-mcp"
DOCKER_SOCKET = "unix:///Users/maleick/.orbstack/run/docker.sock"


def _post(base_url: str, path: str, payload: dict, timeout: int = 300) -> dict:
    body = json.dumps(payload).encode()
    req = urllib.request.Request(
        f"{base_url}{path}",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return {"status": resp.status, "body": resp.read().decode()}
    except urllib.error.HTTPError as exc:
        return {"status": exc.code, "body": exc.read().decode()}


def load_program(
    base_url: str,
    file_path: str,
    language: str = "x86:LE:64:default",
    compiler: str = "windows",
) -> dict:
    return _post(
        base_url,
        "/load_program",
        {"file": file_path, "language": language, "compiler": compiler},
    )


def open_program(base_url: str, program_name: str) -> dict:
    """Activate an already-imported program as the current MCP context."""
    return _post(base_url, "/open_program", {"name": program_name})


def check_context(base_url: str) -> dict | None:
    """Return current-program info from /programs, or None on error."""
    try:
        with urllib.request.urlopen(f"{base_url}/programs", timeout=10) as resp:
            return json.loads(resp.read().decode())
    except Exception:
        return None


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Load and activate a binary in Ghidra MCP"
    )
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--container", default=DEFAULT_CONTAINER)
    parser.add_argument(
        "host_path",
        nargs="?",
        help="Path to binary on host (omit with --open-only)",
    )
    parser.add_argument("--container-path", default="/data/eqgame.exe")
    parser.add_argument("--language", default="x86:LE:64:default")
    parser.add_argument("--compiler", default="windows")
    parser.add_argument(
        "--open-only",
        action="store_true",
        help="Skip docker cp + /load_program; just activate via /open_program",
    )
    parser.add_argument(
        "--program-name",
        default="eqgame.exe",
        help="Program name used in /open_program call (default: eqgame.exe)",
    )
    args = parser.parse_args()

    if not args.open_only and not args.host_path:
        parser.error("host_path is required unless --open-only is set")

    if not args.open_only:
        print(f"Copying {args.host_path} to {args.container}:{args.container_path}")
        subprocess.run(
            [
                "docker",
                "-H",
                DOCKER_SOCKET,
                "cp",
                args.host_path,
                f"{args.container}:{args.container_path}",
            ],
            check=True,
        )

        print(f"Loading {args.container_path} into Ghidra MCP...")
        result = load_program(
            args.base_url, args.container_path, args.language, args.compiler
        )
        print(f"Load result ({result['status']}): {result['body']}")

        if result["status"] not in (200, 201) and '"success": true' not in result["body"]:
            print("Load step failed — aborting.")
            return 1

    print(f"Opening '{args.program_name}' as active context...")
    open_result = open_program(args.base_url, args.program_name)
    print(f"Open result ({open_result['status']}): {open_result['body']}")

    if open_result["status"] not in (200, 201) and '"success": true' not in open_result["body"]:
        print(
            "WARNING: /open_program call failed or returned unexpected status.\n"
            "The server may not support this endpoint; try querying manually:\n"
            f'  curl -s "{args.base_url}/programs"\n'
            f'  curl -s "{args.base_url}/list_functions?limit=5"'
        )
        return 1

    ctx = check_context(args.base_url)
    if ctx:
        current = ctx.get("current_program", "<unknown>")
        print(f"Context confirmed: current_program = {current}")
    else:
        print("Could not verify context via /programs (non-fatal).")

    print(f'Query functions with: curl -s "{args.base_url}/list_functions?limit=5"')
    return 0


if __name__ == "__main__":
    sys.exit(main())
