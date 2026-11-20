"""WHY NO WINDOW (found-defects 824): what a live wait for the program's window says when it is
still waiting or gives up.

A stand and a scenario's own process wait for the window up to the test's guard. One run waited
the whole 600 s on four stands with an empty log, and nothing said whether the program had died,
never reached its display or was still starting. This names it:

- `start_line` — the display, the pid and the program's first log line, written at the start;
- `state` — the supervisor and its editor child alive or not, which of them the display's X server
  counts as its clients (XRes `query_client_ids` by pid), the log's first line;
- `dead` — the program can no longer bring a window: the supervisor ended, or the editor child
  ended and its supervisor stands (it shows the crash window, which is not the program's window).
"""
import os


def _children(pid):
    """The direct children of `pid` (Linux `/proc/<pid>/task/*/children`)."""
    out = []
    try:
        for t in os.listdir(f"/proc/{pid}/task"):
            try:
                with open(f"/proc/{pid}/task/{t}/children") as f:
                    out += [int(c) for c in f.read().split()]
            except OSError:
                pass
    except OSError:
        pass
    return out


def _alive(pid):
    try:
        with open(f"/proc/{pid}/stat") as f:
            return f.read().split(")")[-1].split()[0] != "Z"
    except OSError:
        return False


def first_line(log_path, offset=0):
    """The first non-empty line the program wrote after `offset` (a log opened for appending)."""
    try:
        with open(log_path, errors="replace") as f:
            f.seek(offset)
            for line in f:
                if line.strip():
                    return line.strip()[:200]
    except OSError:
        pass
    return "(empty)"


def x_clients(display):
    """The pids the X server of `display` counts as its clients; `None` when it cannot be asked."""
    try:
        from Xlib import display as xdisplay
        from Xlib.ext import res

        d = xdisplay.Display(display)
        try:
            if not d.has_extension("X-Resource"):
                return None
            r = d.res_query_client_ids([{"client": 0, "mask": res.LocalClientPIDMask}])
            return {int(i.value[0]) for i in r.ids if i.value}
        finally:
            d.close()
    except Exception:
        return None


def start_line(display, pid, log_path, offset=0):
    return f"started on {display}: pid {pid}, log: {first_line(log_path, offset)}"


def state(display, proc, log_path, seen_child, offset=0):
    """One line: what the program is doing while no window is found. `seen_child` — a list the caller
    keeps across calls: the editor child's pid once seen."""
    pid = proc.pid
    if proc.poll() is not None:
        return f"the program ended (code {proc.returncode}); log: {first_line(log_path, offset)}"
    kids = [c for c in _children(pid) if _alive(c)]
    if kids and not seen_child:
        seen_child.append(kids[0])
    clients = x_clients(display)
    mine = {pid, *kids}
    if seen_child and not kids:
        where = "the editor process ended, its supervisor stands (the crash window?)"
    elif clients is None:
        where = f"still starting (the X server of {display} cannot be asked for its clients)"
    elif not clients & mine:
        where = f"not connected to its display {display} (X clients there: {len(clients)})"
    else:
        where = f"connected to {display}, no window yet (still starting)"
    return f"pid {pid}, editor {kids or '-'}: {where}; log: {first_line(log_path, offset)}"


def dead(proc, seen_child):
    """The program can bring no window any more."""
    if proc.poll() is not None:
        return True
    kids = [c for c in _children(proc.pid) if _alive(c)]
    if kids and not seen_child:
        seen_child.append(kids[0])
    return bool(seen_child) and not kids
