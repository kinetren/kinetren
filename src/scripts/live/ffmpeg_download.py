#!/usr/bin/env python3
"""Live scenario ffmpeg-download-window-info: the FFmpeg window tells what Download does.

The owner (kkt 2026-10-27, the second item 3): the FFmpeg download window shows too little —
where it downloads from, how fast, how long, where it puts it. The Windows offer is forced by
the debug seam `KT_FFMPEG_OS=windows`, the mirrors are two local servers (`KT_FFMPEG_DOWNLOAD_URL`
holds both): the first answers 404, the second announces a size other than the expected one.
The SUCCESS state (installed into …, SHA-256 matches, Open folder on it) is t952's: building the
t848-shaped 7z archive needs the Rust writer, which this script does not have.

PATH (ui-reach spec, the FFmpeg download window):
1. View → FFmpeg… → the "what happens" block: the archive, its size, the host and the mirror,
   the start of the checksum, the folder and how to remove it; Download and Open folder below;
2. Download → both mirrors fail: the window says it was not installed and lists under "Tried:"
   each mirror with its own reason (404; "announced … bytes, expected …"), the bar is gone;
3. Open folder stays in the row (hovered here: a click would launch the real file manager).
"""
import http.server
import os
import sys
import threading

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

# the seams are read at start-up: the scenario owns its process
OWN_PROCESS = True


def server(status, body, answer=None):
    """A local server answering every GET with `status` and `body` — once `answer` is set, the way
    a slow network answers, for as long as the scenario needs to see the wait (three seconds were
    gone before a busy machine reached the bar); returns its URL and the server."""

    class Handler(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            if answer is not None:
                answer.wait()
            self.send_response(status)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):
            pass

    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return f"http://127.0.0.1:{srv.server_address[1]}/ffmpeg.7z", srv


def main(out):
    require_tools()
    answer = threading.Event()
    missing, s1 = server(404, b"", answer)
    short, s2 = server(200, b"x" * 100)
    env = {
        "KT_FFMPEG_OS": "windows",
        "KT_FFMPEG_DOWNLOAD_URL": f"{missing} {short}",
        "KT_FFMPEG_DOWNLOAD_SHA256": "0" * 64,
        "KT_FFMPEG_DOWNLOAD_SIZE": "1000",
    }
    was = {k: os.environ.get(k) for k in env}
    os.environ.update(env)
    proj = project_with_clips(os.path.join(out, "ffmpeg_download.otio"), [(0.0, 2.0, "synth:Ramp")])
    try:
        with Live("ffmpeg_download", out, project=proj, own_process=True) as app:
            # 1) before: what Download will do
            app.click_probe("menu.file", "the File menu")
            app.click_probe("file.preferences", "File → Preferences…")
            app.click_probe("prefs.ffmpeg", "Preferences → FFmpeg…")
            app.expect(app.has_probe("ffmpeg.what"), "the window says what Download does")
            app.expect(app.has_probe("ffmpeg.download"), "Download is offered (Windows rules)")
            app.step("before", "the FFmpeg window: what Download gets, from where, into which folder")

            # 2) both mirrors fail: each with its reason
            app.click_probe("ffmpeg.download", "Download")
            # the first mirror is slow to answer: the bar says which host and mirror it waits for
            app.step("connecting", "the bar: “127.0.0.1 — mirror 1 of 2, connecting…”")
            app.hover_probe("ffmpeg.progress", "the progress bar while the first mirror connects")
            answer.set()
            app.expect(app.wait_gone("ffmpeg.progress"), "the bar is gone after the failure")
            app.expect(app.has_probe("ffmpeg.tried"), "the tried mirrors are listed")
            app.expect(app.has_probe("ffmpeg.download"), "Download is offered again")
            app.step("failed", "both mirrors failed: “Tried:” with 404 and the size mismatch")

            # 3) Open folder stays in the row
            app.hover_probe("ffmpeg.open_folder", "Open folder")
            app.step("open-folder", "Open folder in the actions row")
            return app.done()
    finally:
        answer.set()
        s1.shutdown()
        s2.shutdown()
        for k, v in was.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))
