#!/usr/bin/env python3
"""Live scenario of change telemetry-identify-activity: the real program reaches PostHog in
the right shape, and the first-start entry is one calm line in the bell with the lists under
a collapsed "Details".

The owner's words (2026-10-28): "a simple test that the whole thing basically works against a
local PostHog". A real self-hosted PostHog (ClickHouse, Kafka, Postgres, Redis) is far too
heavy for the live run, so a local HTTP server plays its capture endpoint and refuses what the
ingestion would refuse — the same checks as `telemetry::testing::validate` (t888). A debug
build sends to it only when BOTH `KT_TELEMETRY_URL` and `KT_TELEMETRY_KEY` are set (a release
build ignores both); the rig's own `XDG_DATA_HOME` keeps the install id and the instance locks
out of the developer's data directory.
"""
import http.server
import json
import os
import re
import sys
import tempfile
import threading
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, project_with_clips, require_tools  # noqa: E402

# its own process: `app_started` is sent once per process, on the first frame of a fresh
# start, with the target read at process start — the stand cannot be handed either
OWN_PROCESS = True

UUID7 = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[0-9a-f]{4}-[0-9a-f]{12}$")


def validate(path, body):
    """What the ingestion checks (the twin of `telemetry::testing::validate`)."""
    if path != "/i/v0/e/":
        return None, f"not the capture endpoint: {path}"
    try:
        v = json.loads(body)
    except ValueError as e:
        return None, f"not JSON: {e}"
    for k in ("api_key", "event", "distinct_id"):
        if not isinstance(v.get(k), str) or not v[k]:
            return None, f"{k} missing or empty"
    p = v.get("properties")
    if not isinstance(p, dict):
        return None, "properties is not an object"
    if not UUID7.match(str(p.get("$session_id", ""))):
        return None, f"$session_id is not a UUID v7: {p.get('$session_id')!r}"
    for k in ("$set", "$set_once"):
        if k in p and not isinstance(p[k], dict):
            return None, f"{k} is not an object"
    return v, None


class Capture(http.server.BaseHTTPRequestHandler):
    accepted = []
    rejected = []

    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(n).decode("utf-8", "replace")
        v, err = validate(self.path, body)
        if v is None:
            Capture.rejected.append(err)
            out, code = b"{}", 400
        else:
            Capture.accepted.append(v)
            out, code = b'{"status":1}', 200
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)

    def log_message(self, *_args):
        pass


def events():
    return [e["event"] for e in Capture.accepted]


def wait_until(ok, timeout=15.0):
    t0 = time.time()
    while time.time() - t0 < timeout:
        if ok():
            return True
        poll_step(0.2)
    return ok()


def probes(g):
    return {f["param"] for f in g.get("fields", [])}


def main(out):
    require_tools()
    server = http.server.HTTPServer(("127.0.0.1", 0), Capture)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    os.environ["KT_TELEMETRY_URL"] = f"http://127.0.0.1:{server.server_port}/i/v0/e/"
    os.environ["KT_TELEMETRY_KEY"] = "local-stand-in"
    # the entry's words are checked in the catalog's reference language
    os.environ["KT_LANG"] = "en"
    src = os.path.join(tempfile.mkdtemp(prefix="kt-live-telemetry-"), "telemetry.otio")
    project_with_clips(src, [(0.0, 6.0)])
    try:
        with Live("telemetry", out, project=src, own_process=True) as app:
            # start: nothing pops up, the bell holds one entry, start + project reached PostHog
            wait_until(lambda: "project_opened" in events())
            g = app.geom()
            notes = [n for n in g.get("notices", []) if "anonymous usage statistics" in n]
            app.expect(len(notes) == 1, f"one calm entry in the bell: {g.get('notices')}")
            app.expect(
                all("File / Preferences… / Send anonymous usage statistics" in n for n in notes),
                f"the entry names where it is switched off: {notes}",
            )
            app.expect(all("NOT sent" not in n for n in notes), f"the lists are not in the line: {notes}")
            app.expect(int(g.get("windows", 0)) == 0, f"no window opened at start: {g.get('windows')}")
            app.expect(events()[:2] == ["app_started", "project_opened"], f"start, then the project: {events()}")
            app.step("start", "nothing pops up; the bell shows 1")

            # two seconds of playback: counted, not narrated
            app.key("space")
            app.wait_playhead(lambda t: t > 27_000, "the playhead runs after Space")
            poll_step(2.0)
            app.key("space")
            app.step("played", "Space, two seconds, Space: the playhead stopped")

            # the bell: one calm line, "Details" collapsed; a click opens the lists
            app.click_probe("bell", "the bell")
            wait_until(lambda: int(app.geom().get("windows", 0)) == 1, timeout=5.0)
            app.expect(app.has_probe("notices.details"), "the entry has a “Details” header")
            app.expect(not app.has_probe("notices.details_body", tries=2), "“Details” is collapsed")
            app.step("bell", "one calm line with “Details” collapsed")
            app.click_probe("notices.details", "“Details”")
            app.expect(app.has_probe("notices.details_body"), "“Details” opens")
            app.step("details", "what is sent, where it goes, and what is NOT sent")

            # the window's cross: the clean close sends app_closed within its budget
            app.close_window()
            app.expect(wait_until(lambda: "app_closed" in events(), timeout=10.0), f"app_closed arrived: {events()}")
            ev = {e["event"]: e for e in Capture.accepted}
            app.expect(not Capture.rejected, f"the stand-in refused nothing: {Capture.rejected}")
            app.expect(
                [e for e in events() if e != "app_heartbeat"] == ["app_started", "project_opened", "app_closed"],
                f"the session's events: {events()}",
            )
            started = ev.get("app_started", {}).get("properties", {})
            app.expect(isinstance(started.get("$set"), dict) and "$app_version" in started["$set"], "the start identifies the install")
            closed = ev.get("app_closed", {}).get("properties", {})
            app.expect(closed.get("playback_starts", 0) >= 1, f"the close counts the playback: {closed}")
            app.expect(len({e["properties"]["$session_id"] for e in Capture.accepted}) == 1, "one session id")
            app.expect(len({e["distinct_id"] for e in Capture.accepted}) == 1, "one install id")
            blob = json.dumps(Capture.accepted)
            app.expect("telemetry.otio" not in blob and "/tmp/" not in blob, "no path or file name left the machine")
            return app.done()
    finally:
        server.shutdown()
        for k in ("KT_TELEMETRY_URL", "KT_TELEMETRY_KEY", "KT_LANG"):
            os.environ.pop(k, None)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
