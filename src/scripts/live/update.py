#!/usr/bin/env python3
"""Live scenario of change update-check: a newer release is said in the bell and as a short
word after the version in the title — never in a window.

The owner's words (2026-10-27): "remind him in the bell notice that the version is old (but
do not bother him with pop-up windows at startup), and also write at the top in the title
bar, somehow briefly, that the version is outdated."

A local HTTP server plays GitHub: it answers the latest-release JSON with an asset dated
99.12.31, which is newer than any real build. A debug build asks it only through
`KT_UPDATE_URL` (a release build ignores the variable); the rig's own `XDG_DATA_HOME` keeps
the day's answer out of the developer's data directory.
"""
import http.server
import json
import os
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, require_tools  # noqa: E402

# its own process: the check runs once, on the first frame of a fresh start, with
# `KT_UPDATE_URL` read at process start — the stand cannot be handed either
OWN_PROCESS = True

NEW = "kt99.12.31"
RELEASE = {
    "tag_name": "v99.12.31",
    "prerelease": False,
    "draft": False,
    "assets": [{"name": "kinetren-99.12.31-0123abcd-linux-x86_64.tar.gz"}],
}


class Releases(http.server.BaseHTTPRequestHandler):
    requests = []

    def do_GET(self):
        Releases.requests.append((self.path, self.headers.get("User-Agent", "")))
        body = json.dumps(RELEASE).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


def wait_for(app, ok, timeout=15.0):
    t0 = time.time()
    while time.time() - t0 < timeout:
        g = app.geom()
        if ok(g):
            return g
        poll_step(0.2)
    return app.geom()


def probes(g):
    return {f["param"] for f in g.get("fields", [])}


def main(out):
    require_tools()
    server = http.server.HTTPServer(("127.0.0.1", 0), Releases)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    os.environ["KT_UPDATE_URL"] = (
        f"http://127.0.0.1:{server.server_port}/repos/kinetren/kinetren/releases/latest"
    )
    try:
        with Live("update", out, own_process=True) as app:
            # start: no window; the bell has the notice, the title row has the word
            g = wait_for(app, lambda g: any(NEW in n for n in g.get("notices", [])))
            notes = [n for n in g.get("notices", []) if NEW in n]
            app.expect(len(notes) == 1, f"one notice names {NEW}: {g.get('notices')}")
            app.expect(
                any("github.com/kinetren/kinetren/releases" in n for n in notes),
                f"the notice says where to get it: {notes}",
            )
            app.expect(int(g.get("windows", 0)) == 0, f"no window opened at start: {g.get('windows')}")
            app.expect("title.outdated" in probes(g), "the word is painted after the version")
            app.expect(len(Releases.requests) == 1, f"one request: {Releases.requests}")
            path, agent = Releases.requests[0] if Releases.requests else ("", "")
            app.expect(
                path == "/repos/kinetren/kinetren/releases/latest" and agent.startswith("kinetren/kt"),
                f"the request names only the program version: {path} {agent}",
            )
            app.step("start", "no window; the bell shows 1, the title reads “… (outdated)”")

            # hovering the word says which version is out and where, in the status line
            app.hover_probe("title.outdated", "the word in the title row")
            g = wait_for(app, lambda g: NEW in g.get("status", ""), timeout=5.0)
            app.expect(NEW in g.get("status", ""), f"the hint names {NEW}: {g.get('status')}")
            app.step("hover-word", "the status line says which version is out and where")

            # View: the switch under the telemetry one; unchecked, the word leaves the title
            app.click_probe("menu.view", "“View” menu")
            app.settle()
            app.hover_probe("view.update_check", "“Check for new versions daily”")
            g = wait_for(app, lambda g: "GitHub" in g.get("status", ""), timeout=5.0)
            app.expect("GitHub" in g.get("status", ""), f"the switch says what it asks: {g.get('status')}")
            app.step("view-menu", "View: “Check for new versions daily” is checked")
            app.click_probe("view.update_check", "uncheck “Check for new versions daily”")
            app.key("Escape")
            g = wait_for(app, lambda g: "title.outdated" not in probes(g), timeout=5.0)
            app.expect("title.outdated" not in probes(g), "the word left the title")
            app.step("unchecked", "the title is plain again")
            # the bell opens the notices: the person's click, the only window here
            app.click_probe("bell", "the bell")
            g = wait_for(app, lambda g: int(g.get("windows", 0)) == 1, timeout=5.0)
            app.expect(int(g.get("windows", 0)) == 1, "the bell opens the notices")
            app.step("notices", "the notice: the new version, ours, the address, the View item (the word is gone, the notice stays)")

            app.expect(len(Releases.requests) == 1, f"still one request: {Releases.requests}")
            return app.done()
    finally:
        server.shutdown()
        os.environ.pop("KT_UPDATE_URL", None)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
