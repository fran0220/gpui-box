#!/usr/bin/env python3

import argparse
import contextlib
import http.server
import os
import pathlib
import shutil
import struct
import subprocess
import tempfile
import threading
import urllib.request
import zlib


ROOT = pathlib.Path(__file__).resolve().parent


class SmokeHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=ROOT / "dist", **kwargs)

    def end_headers(self):
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, format, *args):
        pass


def find_chrome(explicit_path):
    if explicit_path:
        return explicit_path
    for name in ("google-chrome", "chromium", "chromium-browser", "chrome"):
        path = shutil.which(name)
        if path:
            return path
    candidates = (
        pathlib.Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
        pathlib.Path(os.environ.get("PROGRAMFILES", "")) / "Google/Chrome/Application/chrome.exe",
        pathlib.Path(os.environ.get("PROGRAMFILES(X86)", "")) / "Google/Chrome/Application/chrome.exe",
    )
    return next((str(path) for path in candidates if path.is_file()), None)


def screenshot_contains_clicked_view(path):
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise RuntimeError("Chrome screenshot is not a PNG")
    offset = 8
    compressed = bytearray()
    width = height = color_type = None
    while offset < len(data):
        length = struct.unpack(">I", data[offset : offset + 4])[0]
        chunk_type = data[offset + 4 : offset + 8]
        chunk = data[offset + 8 : offset + 8 + length]
        offset += length + 12
        if chunk_type == b"IHDR":
            width, height, bit_depth, color_type = struct.unpack(">IIBB", chunk[:10])
            if bit_depth != 8 or color_type not in (2, 6):
                raise RuntimeError("unsupported Chrome screenshot PNG format")
        elif chunk_type == b"IDAT":
            compressed.extend(chunk)
        elif chunk_type == b"IEND":
            break

    channels = 3 if color_type == 2 else 4
    stride = width * channels
    raw = zlib.decompress(compressed)
    previous = bytearray(stride)
    green_pixels = 0
    dark_pixels = 0
    position = 0
    for _ in range(height):
        filter_type = raw[position]
        position += 1
        row = bytearray(raw[position : position + stride])
        position += stride
        for index in range(stride):
            left = row[index - channels] if index >= channels else 0
            above = previous[index]
            upper_left = previous[index - channels] if index >= channels else 0
            if filter_type == 1:
                row[index] = (row[index] + left) & 0xFF
            elif filter_type == 2:
                row[index] = (row[index] + above) & 0xFF
            elif filter_type == 3:
                row[index] = (row[index] + ((left + above) // 2)) & 0xFF
            elif filter_type == 4:
                estimate = left + above - upper_left
                nearest = min((left, above, upper_left), key=lambda value: abs(estimate - value))
                row[index] = (row[index] + nearest) & 0xFF
            elif filter_type != 0:
                raise RuntimeError(f"unsupported PNG filter {filter_type}")
        for index in range(0, stride, channels):
            red, green, blue = row[index : index + 3]
            if abs(red - 0xA6) <= 2 and abs(green - 0xE3) <= 2 and abs(blue - 0xA1) <= 2:
                green_pixels += 1
            if abs(red - 0x1E) <= 2 and abs(green - 0x1E) <= 2 and abs(blue - 0x2E) <= 2:
                dark_pixels += 1
        previous = row
    return green_pixels >= 50_000 and dark_pixels >= 20


def run_case(chrome, base_url, query, expected_renderer, disable_webgpu=False, test_accessibility=False):
    with tempfile.TemporaryDirectory(prefix="gpui-web-chrome-") as profile:
        screenshot = pathlib.Path(profile) / "smoke.png"
        base_command = [
            chrome,
            "--headless=new",
            "--no-sandbox",
            "--enable-unsafe-swiftshader",
            "--disable-background-networking",
            "--disable-component-update",
            "--disable-default-apps",
            "--disable-sync",
            "--force-device-scale-factor=2",
            "--metrics-recording-only",
            "--no-first-run",
            f"--user-data-dir={profile}",
        ]
        if disable_webgpu:
            base_command.append("--disable-features=WebGPU")
        url = f"{base_url}/?smoke=1&{query}"
        command = [
            *base_command,
            "--virtual-time-budget=30000",
            "--dump-dom",
            url,
        ]
        result = subprocess.run(command, capture_output=True, text=True, timeout=45)
        output = result.stdout
        if result.returncode != 0 or 'data-gpui-smoke-result="pass"' not in output:
            error = next(
                (
                    line.strip()
                    for line in output.splitlines()
                    if "data-gpui-smoke-error=" in line or "data-gpui-smoke-phase=" in line
                ),
                "no browser smoke result",
            )
            raise RuntimeError(
                f"{query}: {error}\nDOM tail:\n{output[-2000:]}\nChrome tail:\n{result.stderr[-2000:]}"
            )
        required = (
            'data-gpui-smoke="clicked"',
            'data-gpui-smoke-clicks="1"',
            'data-gpui-painted="true"',
            f'data-gpui-renderer="{expected_renderer}"',
            'data-gpui-isolated="true"',
        )
        if test_accessibility:
            required = (*required, 'data-gpui-a11y="pass"')
        missing = [attribute for attribute in required if attribute not in output]
        if missing:
            raise RuntimeError(f"{query}: missing browser evidence: {', '.join(missing)}")
        screenshot_result = subprocess.run(
            [
                *base_command,
                "--virtual-time-budget=12000",
                f"--screenshot={screenshot}",
                url,
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if screenshot_result.returncode != 0:
            raise RuntimeError(f"{query}: Chrome screenshot failed: {screenshot_result.stderr[-2000:]}")
        if not screenshot.is_file() or not screenshot_contains_clicked_view(screenshot):
            raise RuntimeError(f"{query}: Chrome screenshot does not contain the clicked GPUI div/text")


def main():
    parser = argparse.ArgumentParser(description="Run hello_web in real headless Chrome")
    parser.add_argument("--chrome", help="path to Chrome or Chromium")
    parser.add_argument("--no-build", action="store_true", help="reuse the existing dist directory")
    parser.add_argument(
        "--webgpu",
        action="store_true",
        help="also require forced WebGPU and Auto-to-WebGPU rendering",
    )
    args = parser.parse_args()

    chrome = find_chrome(args.chrome)
    if not chrome:
        raise SystemExit("Chrome/Chromium not found; pass --chrome")

    if not args.no_build:
        environment = os.environ.copy()
        environment["NO_COLOR"] = "true"
        subprocess.run(["trunk", "build"], cwd=ROOT, env=environment, check=True)

    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), SmokeHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        base_url = f"http://127.0.0.1:{server.server_port}"
        with contextlib.closing(urllib.request.urlopen(base_url)) as response:
            if response.headers.get("Cross-Origin-Embedder-Policy") != "require-corp":
                raise RuntimeError("test server did not send COEP")
            if response.headers.get("Cross-Origin-Opener-Policy") != "same-origin":
                raise RuntimeError("test server did not send COOP")

        run_case(chrome, base_url, "backend=webgl&a11y=1", "webgl2", test_accessibility=True)
        print("PASS forced WebGL: rendered div/text, retained canvas and pointer click")
        run_case(chrome, base_url, "backend=auto", "webgl2", disable_webgpu=True)
        print("PASS Auto with WebGPU disabled -> WebGL: rendered div/text, retained canvas, pointer click")
        if args.webgpu:
            run_case(chrome, base_url, "backend=webgpu", "webgpu")
            print("PASS forced WebGPU: rendered div/text, retained canvas, pointer click")
            run_case(chrome, base_url, "backend=auto", "webgpu")
            print("PASS Auto -> WebGPU: rendered div/text, retained canvas, pointer click")
        print("PASS accessibility mirror: role, focus ownership and canvas-adjusted bounds")
        print("PASS response headers: COOP/COEP; crossOriginIsolated=true")
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


if __name__ == "__main__":
    main()
