#!/usr/bin/env python3
"""ДИАГНОСТИЧЕСКИЙ прогон (Р.31 п.2, не входит в регулярный набор из-за
префикса zz_? — входит; удалить после расследования): 75 секунд простоя,
по /proc считаем CPU-время каждого потока за каждую секунду. Ловим
периодические всплески «раз в ~30 с»."""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402


def thread_times(pid):
    out = {}
    base = f"/proc/{pid}/task"
    try:
        for tid in os.listdir(base):
            try:
                with open(f"{base}/{tid}/stat") as f:
                    parts = f.read().rsplit(")", 1)[1].split()
                name = open(f"{base}/{tid}/comm").read().strip()
                out[tid] = (name, int(parts[11]) + int(parts[12]))  # utime+stime
            except OSError:
                pass
    except OSError:
        pass
    return out


def main(out):
    require_tools()
    media = color_media(out, "gray", "D0D0D0", seconds=10)
    path = project_with_clips(os.path.join(out, "idle.otio"), [(0.0, 8.0, media)])
    with Live("zz_idleprofile", out, project=path) as app:
        pid = app.proc.pid
        prev = thread_times(pid)
        t0 = time.time()
        print("   сек: потоки с > 20 тиков CPU за секунду")
        while time.time() - t0 < 75:
            time.sleep(1.0)
            cur = thread_times(pid)
            spikes = []
            for tid, (name, tt) in cur.items():
                d = tt - prev.get(tid, (name, tt))[1]
                if d > 20:  # >0.2 c CPU за секунду
                    spikes.append(f"{name}={d}")
            if spikes:
                print(f"   {int(time.time()-t0):3d}s  " + " ".join(sorted(spikes)))
            prev = cur
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))
