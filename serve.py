# SPDX-License-Identifier: MIT OR Apache-2.0
# Copyright (c) 2026 R.F. van Ee
"""Development web server for web/ that tells the browser not to cache.

`python -m http.server` sends no cache headers, so after a rebuild a normal
reload can still run the previous app.js or pkg/c64_wasm_bg.wasm. Use this
instead:  python serve.py [port]   (default 8000), then open
http://localhost:8000/

It also serves /disks/ from crates/c64-core/test-disks/ (the GitHub Pages
build copies them there), so the page's "test disk" links work locally.
"""
import functools
import http.server
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
WEB = os.path.join(HERE, "web")
DISKS = os.path.join(HERE, "crates", "c64-core", "test-disks")


class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".wasm": "application/wasm",
        ".js": "text/javascript",
        ".mjs": "text/javascript",
        ".d64": "application/octet-stream",
    }

    def translate_path(self, path):
        clean = path.split("?", 1)[0].split("#", 1)[0]
        if clean.startswith("/disks/") and not os.path.isdir(os.path.join(WEB, "disks")):
            return os.path.join(DISKS, os.path.basename(clean))
        return super().translate_path(path)

    def end_headers(self):
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
    handler = functools.partial(NoCacheHandler, directory=WEB)
    print(f"serving {WEB} on http://localhost:{port}/ (no caching)")
    http.server.ThreadingHTTPServer(("", port), handler).serve_forever()
