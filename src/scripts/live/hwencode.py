#!/usr/bin/env python3
"""Live scenario hw-encode-context: a hardware encoder that cannot work never ends the render.

The owner (ktprompts v26.10 T16): "if you choose h264_vaapi the app just crashes at render". This
machine has no graphics card, so the driver is OUR fake one (`fixtures/fake_va`) under the real
libva and FFmpeg: it proves the path, not a real driver. The project is a synthetic fixture.

PATH (export spec):
1. a driver that crashes in the encoder's open: File → Render… → the settings window;
2. the "Video codec" list → h264_vaapi with its device → Render…;
3. the window ends with the error, Open log and Close; no file; the editor is alive;
4. Close → the settings again → Auto → Render… → the render finishes on the next encoder;
5. a healthy driver: the render on Auto finishes on h264_vaapi, Open file offered.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, RUST, project_with_clips, require_tools  # noqa: E402

# the driver and the save seam are read at start-up: the scenario owns its processes
OWN_PROCESS = True


def main(out):
    require_tools()
    os.makedirs(out, exist_ok=True)
    va = os.path.join(out, "fake_va")
    dri = os.path.join(va, "dri")
    os.makedirs(dri, exist_ok=True)
    with open(os.path.join(dri, "renderD128"), "wb") as f:
        f.write(b"a regular file")
    driver = os.path.join(va, "ktfake_drv_video.so")
    subprocess.check_call(
        ["cc", "-shared", "-fPIC", "-O1", "-w", "-o", driver,
         os.path.join(RUST, "fixtures", "fake_va", "ktfake_drv_video.c")]
    )
    target = os.path.join(out, "kt-hw.mp4")
    if os.path.exists(target):
        os.remove(target)
    env = {
        "KT_FILE_PICK": target,
        "KT_DRI_DIR": dri,
        "LD_PRELOAD": driver,
        "LIBVA_DRIVERS_PATH": va,
        "LIBVA_DRIVER_NAME": "ktfake",
    }
    was = {k: os.environ.get(k) for k in [*env, "KT_FAKE_VA"]}
    os.environ.update(env)
    proj = project_with_clips(os.path.join(out, "hw.otio"), [(0.0, 2.0, "synth:Ramp")])

    def replace_if_asked(app):
        if app.has_probe("ask.replace", tries=4):
            app.click_probe("ask.replace", "Replace")

    try:
        os.environ["KT_FAKE_VA"] = "context-abort"
        with Live("hwencode", out, project=proj, own_process=True) as app:
            # 1–2) the choice that crashed the program
            app.click_probe("menu.file", "the File menu")
            app.click_probe("file.render", "File → Render…")
            app.expect(app.has_probe("render.start"), "the settings window")
            app.click_probe("render.encoder", "the Video codec list")
            app.expect(app.has_probe("render.encoder.h264_vaapi"), "the device opens, so h264_vaapi is offered")
            app.step("list", "the list offers h264_vaapi with its device")
            app.click_probe("render.encoder.h264_vaapi", "h264_vaapi")
            app.click_probe("render.start", "Render…")

            # 3) an error in the window, the program alive
            app.expect(app.has_probe("render.open_log", tries=120), "the window ends with the error")
            app.expect(not os.path.exists(target), "no file was left")
            app.step("error", "the render failed with the reason: the driver crashed in a test process")

            # 4) the same driver on Auto: the render comes out
            app.click_probe("render.close", "Close")
            app.expect(app.wait_gone("render.close"), "the window closed")
            app.click_probe("menu.file", "the File menu")
            app.click_probe("file.render", "File → Render…")
            app.click_probe("render.encoder", "the Video codec list")
            app.click_probe("render.encoder.libx264", "libx264")
            app.click_probe("render.start", "Render…")
            replace_if_asked(app)
            app.expect(app.has_probe("render.open_file", tries=240), "the render finished on software")
            app.step("software", "the render finished, the editor never left")
            app.click_probe("render.close", "Close")
            app.expect(app.wait_gone("render.close"), "the window closed")
            app.done()

        # 5) a healthy driver: the hardware encoder on Auto
        os.environ["KT_FAKE_VA"] = ""
        os.remove(target)
        with Live("hwencode-healthy", out, project=proj, own_process=True) as app:
            app.click_probe("menu.file", "the File menu")
            app.click_probe("file.render", "File → Render…")
            app.click_probe("render.start", "Render…")
            replace_if_asked(app)
            app.expect(app.has_probe("render.open_file", tries=240), "the render finished on hardware")
            app.step("hardware", "Video: h264_vaapi (hardware · the device) · nv12 · QP 23")
            with open(target, "rb") as f:
                app.expect(f.read(12)[4:8] == b"ftyp", "the file on disk is an MP4")
            return app.done()
    finally:
        for k, v in was.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))
