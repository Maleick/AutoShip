#!/usr/bin/env python3
"""Load new binary into Ghidra MCP for offset research."""
import argparse
import subprocess
import sys
import urllib.request
import urllib.parse


DEFAULT_BASE_URL = "http://127.0.0.1:8089"
DEFAULT_CONTAINER = "ghidra-mcp"
DOCKER_SOCKET = "unix:///Users/maleick/.orbstack/run/docker.sock"


def load_program(base_url: str, file_path: str, language: str = "x86:LE:64:default", compiler: str = "windows") -> dict:
    body = f'{{"file": "{file_path}", "language": "{language}", "compiler": "{compiler}"}}'.encode()
    req = urllib.request.Request(
        f"{base_url}/load_program",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=300) as resp:
        return {"status": resp.status, "body": resp.read().decode()}


def main() -> int:
    parser = argparse.ArgumentParser(description="Load binary into Ghidra MCP")
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--container", default=DEFAULT_CONTAINER)
    parser.add_argument("host_path", help="Path to binary on host")
    parser.add_argument("--container-path", default="/data/eqgame.exe")
    parser.add_argument("--language", default="x86:LE:64:default")
    parser.add_argument("--compiler", default="windows")
    args = parser.parse_args()

    container_path = args.container_path

    print(f"Copying {args.host_path} to {args.container}:{container_path}")
    subprocess.run(
        ["docker", "-H", DOCKER_SOCKET, "cp", args.host_path, f"{args.container}:{container_path}"],
        check=True,
    )

    print(f"Loading {container_path} into Ghidra MCP...")
    result = load_program(args.base_url, container_path, args.language, args.compiler)
    print(f"Result: {result['body']}")

    if '"success": true' in result["body"]:
        print("Successfully loaded binary.")
        print(f"Query functions with: curl -s \"{args.base_url}/list_functions?program=eqgame.exe\"")
        return 0
    else:
        print(f"Failed to load: {result['body']}")
        return 1


if __name__ == "__main__":
    sys.exit(main())