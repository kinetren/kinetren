#!/usr/bin/env python3
"""Guard of the repository's public face (readme-showcase, t826/t827).

    python3 src/scripts/readme-check.py --check            # this repository
    python3 src/scripts/readme-check.py --check --root DIR # another tree (tests)

Checks README.md, CONTRIBUTING.md and docs/guide/**/*.md:

  * cyrillic   — no Cyrillic characters (the public docs are English);
  * link       — every relative link resolves to a file, and a `#anchor`
                 to a heading of that file (GitHub slug rules);
  * image      — every referenced image exists under docs/images/ (the one
                 exception: the application icon from src/packaging/);
  * orphan     — every file under docs/images/ is referenced;
  * size       — each image ≤ 900 KB, docs/images/ in total ≤ 8 MB;
  * shots      — the referenced images are exactly the set declared in
                 `SHOTS` of src/scripts/live/readme_shots.py: an image added
                 by hand, or a declared shot the docs do not show, is red.

Code blocks are skipped. Every violation prints `file:line: class: detail`;
any violation makes `--check` exit 1.
"""
import ast
import os
import re
import sys
import unicodedata
from pathlib import Path

IMAGE_MAX = 900 * 1024
IMAGES_TOTAL_MAX = 8 * 1024 * 1024
IMAGE_EXT = {".webp", ".png", ".jpg", ".jpeg", ".gif", ".svg"}
# the application icon at the top of the README lives with the packaging
ICON_DIR = Path("src") / "packaging"

CYRILLIC = re.compile(r"[Ѐ-ӿԀ-ԯ]")
MD_LINK = re.compile(r"!?\[[^\]]*\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
HTML_SRC = re.compile(r"<(?:img|a)\b[^>]*?\b(?:src|href)\s*=\s*\"([^\"]+)\"", re.I)
HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
FENCE = re.compile(r"^\s*(```|~~~)")


def docs(root):
    out = [root / "README.md", root / "CONTRIBUTING.md"]
    guide = root / "docs" / "guide"
    if guide.is_dir():
        out += sorted(guide.rglob("*.md"))
    return [p for p in out if p.exists()]


def lines_outside_code(text):
    """(line number, line) for every line outside fenced code blocks;
    inline code spans are blanked so `—` examples in them are not links."""
    fenced = False
    for n, line in enumerate(text.split("\n"), 1):
        if FENCE.match(line):
            fenced = not fenced
            continue
        if not fenced:
            yield n, re.sub(r"`[^`]*`", "``", line)


def slug(heading):
    """GitHub's anchor for a heading: lower case, punctuation dropped,
    spaces to hyphens."""
    s = re.sub(r"<[^>]+>", "", heading)  # inline HTML
    s = re.sub(r"!?\[([^\]]*)\]\([^)]*\)", r"\1", s)  # links keep their text
    s = s.replace("`", "").strip().lower()
    out = []
    for ch in s:
        cat = unicodedata.category(ch)
        if ch in "-_" or ch.isalnum():
            out.append(ch)
        elif ch == " ":
            out.append("-")
        elif cat.startswith("P") or cat.startswith("S"):
            continue
    return "".join(out)


def anchors(path, cache={}):
    if path not in cache:
        seen, found = {}, set()
        text = path.read_text(encoding="utf-8")
        for _, line in lines_outside_code(text):
            m = HEADING.match(line)
            if not m:
                continue
            base = slug(m.group(2))
            k = seen.get(base, 0)
            seen[base] = k + 1
            found.add(base if k == 0 else f"{base}-{k}")
        cache[path] = found
    return cache[path]


def declared_shots(root):
    """Names in `SHOTS = [...]` of the pipeline, without importing it (it
    pulls in the X11 live framework)."""
    src = root / "src" / "scripts" / "live" / "readme_shots.py"
    if not src.exists():
        return None
    tree = ast.parse(src.read_text(encoding="utf-8"))
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
                isinstance(t, ast.Name) and t.id == "SHOTS" for t in node.targets):
            return {name for name, _ in ast.literal_eval(node.value)}
    return None


def check(root):
    root = Path(root).resolve()
    images_dir = root / "docs" / "images"
    problems = []
    referenced = set()

    def bad(path, line, cls, detail):
        rel = path.relative_to(root) if path.is_absolute() else path
        problems.append(f"{rel}:{line}: {cls}: {detail}")

    for doc in docs(root):
        text = doc.read_text(encoding="utf-8")
        for n, line in enumerate(text.split("\n"), 1):
            m = CYRILLIC.search(line)
            if m:
                bad(doc, n, "cyrillic", f"«{line.strip()[:60]}»")
        for n, line in lines_outside_code(text):
            targets = MD_LINK.findall(line) + HTML_SRC.findall(line)
            for target in targets:
                if re.match(r"^[a-z][a-z0-9+.-]*:", target, re.I):
                    continue  # http:, https:, mailto:
                path_part, _, anchor = target.partition("#")
                dest = (doc.parent / path_part).resolve() if path_part else doc
                if path_part and not dest.exists():
                    cls = "image" if Path(path_part).suffix.lower() in IMAGE_EXT else "link"
                    bad(doc, n, cls, f"{target} does not exist")
                    continue
                if dest.is_file() and dest.suffix.lower() in IMAGE_EXT:
                    try:
                        rel = dest.relative_to(images_dir)
                    except ValueError:
                        if dest.parent != (root / ICON_DIR).resolve():
                            bad(doc, n, "image", f"{target} is outside docs/images/")
                        continue
                    referenced.add(rel.as_posix())
                if anchor and dest.suffix == ".md" and anchor not in anchors(dest):
                    bad(doc, n, "link", f"{target}: no heading «#{anchor}» in {dest.name}")

    total = 0
    on_disk = set()
    if images_dir.is_dir():
        for p in sorted(images_dir.rglob("*")):
            if not p.is_file() or p.suffix.lower() not in IMAGE_EXT:
                continue  # CREDITS.md and other notes live next to the images
            rel = p.relative_to(images_dir).as_posix()
            on_disk.add(rel)
            size = p.stat().st_size
            total += size
            if size > IMAGE_MAX:
                bad(p, 0, "size", f"{size // 1024} KB > {IMAGE_MAX // 1024} KB")
            if rel not in referenced:
                bad(p, 0, "orphan", "not referenced from README.md, CONTRIBUTING.md or docs/guide/")
    if total > IMAGES_TOTAL_MAX:
        bad(images_dir, 0, "size", f"docs/images/ is {total // 1024} KB > {IMAGES_TOTAL_MAX // 1024} KB")

    shots = declared_shots(root)
    if shots is not None:
        shown = {Path(r).stem for r in referenced}
        for name in sorted(shots - shown):
            bad(images_dir, 0, "shots", f"declared shot «{name}» is not shown in the docs")
        for name in sorted(shown - shots):
            bad(images_dir, 0, "shots",
                f"«{name}» is not produced by readme_shots.py (images are never added by hand)")
    return problems


def main(argv):
    root = Path(__file__).resolve().parents[2]
    if "--root" in argv:
        root = Path(argv[argv.index("--root") + 1])
    problems = check(root)
    for p in problems:
        print(p)
    if "--check" in argv:
        if problems:
            print(f"readme-check: {len(problems)} problem(s)", file=sys.stderr)
            return 1
        print("readme-check: README, CONTRIBUTING and docs/guide are consistent")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
