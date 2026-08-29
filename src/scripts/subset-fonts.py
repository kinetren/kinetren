#!/usr/bin/env python3
"""Сабсеты шрифтов для мультиязычного интерфейса (Р.19, решение п.2).

Дефолтные шрифты egui (Ubuntu-Light, NotoEmoji, emoji-icon-font, Hack) и
вендоренный Phosphor не содержат НИ ОДНОГО арабского или иероглифического
кодпоинта — без своих шрифтов пользователь увидит квадраты-заглушки. Целиком
Noto Sans CJK весит ~19 МБ, поэтому режем его ровно по символам каталогов
перевода: получается ~200 КБ, и результат коммитится в assets/.

    python3 scripts/subset-fonts.py            # собрать сабсеты
    python3 scripts/subset-fonts.py --check    # только проверить покрытие

Требуется fontTools (pyftsubset). Исходные шрифты в репозиторий НЕ кладутся:
берутся системные Noto, путь можно задать через --cjk/--arabic.
"""

import argparse
import hashlib
import pathlib
import re
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
I18N = ROOT / "apps/kt-app/i18n"
ASSETS = ROOT / "apps/kt-app/assets"

CJK_OUT = ASSETS / "NotoSansSC-kt.otf"
AR_OUT = ASSETS / "NotoSansArabic-kt.ttf"
MANIFEST = ASSETS / "SUBSET-MANIFEST.txt"

CJK_SOURCES = [
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 2),  # face 2 = SC
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-VF.otf.ttc", 2),
]
AR_SOURCES = [
    ("/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf", 0),
    ("/usr/share/fonts/truetype/noto/NotoNaskhArabic-Regular.ttf", 0),
]

# Что рисует какой шрифт. Диапазоны обязаны совпадать с предикатом
# `bidi::is_arabic` в apps/kt-app/src/i18n/bidi.rs — на их точности держится
# переупорядочивание арабского.
CJK_RANGES = [(0x2E80, 0x9FFF), (0x3000, 0x303F), (0x3400, 0x4DBF), (0xFF00, 0xFFEF)]
AR_RANGES = [(0x0600, 0x06FF), (0x0750, 0x077F), (0x08A0, 0x08FF),
             (0xFB50, 0xFDFF), (0xFE70, 0xFEFF)]

# Арабский режем блоком целиком, а не по факту использования: разница ~20 КБ,
# зато сабсет не приходится пересобирать после каждой правки перевода.
AR_SUBSET_RANGE = (0x0600, 0x06FF)

# Соединение вязи живёт в этих фичах: без init/medi/fina/isol буквы рассыплются
# на отдельные формы, и это будет ВИДНО.
AR_FEATURES = "ccmp,locl,init,medi,fina,isol,rlig,liga,calt,mark,mkmk,kern"
# Упрощённому китайскому шейпинг не нужен вовсе.
CJK_FEATURES = ""


def in_ranges(cp, ranges):
    return any(lo <= cp <= hi for lo, hi in ranges)


def catalog_chars():
    """Все символы всех значений всех каталогов."""
    chars = set()
    value = re.compile(r'^\s*[a-z][a-z0-9_]*\s*=\s*"((?:[^"\\]|\\.)*)"')
    for path in sorted(I18N.glob("*.toml")):
        for line in path.read_text(encoding="utf-8").splitlines():
            m = value.match(line)
            if not m:
                continue
            text = (m.group(1).replace("\\n", "\n").replace("\\t", "\t")
                    .replace('\\"', '"').replace("\\\\", "\\"))
            chars.update(text)
    return chars


def split(chars):
    cjk = {c for c in chars if in_ranges(ord(c), CJK_RANGES)}
    ar = {c for c in chars if in_ranges(ord(c), AR_RANGES)}
    rest = chars - cjk - ar
    return cjk, ar, rest


def ubuntu_light_cmap():
    """Кодпоинты дефолтного шрифта egui — всё остальное обязано быть в нём."""
    from fontTools.ttLib import TTFont
    import glob
    hits = glob.glob(str(pathlib.Path.home()
                        / ".cargo/registry/src/*/epaint_default_fonts-*/fonts/Ubuntu-Light.ttf"))
    if not hits:
        return None
    return set(TTFont(hits[0]).getBestCmap())


def pick(sources, what):
    for path, index in sources:
        if pathlib.Path(path).exists():
            return pathlib.Path(path), index
    sys.exit(f"no source font for {what}; pass an explicit path (see --help)")


def sha(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()[:16]


def subset(src, index, unicodes, features, out):
    with tempfile.TemporaryDirectory() as tmp:
        tmp_out = pathlib.Path(tmp) / "out.ttf"
        cmd = [
            sys.executable, "-m", "fontTools.subset", str(src),
            f"--font-number={index}",
            "--unicodes=" + ",".join(f"U+{u:04X}" for u in sorted(unicodes)),
            f"--layout-features={features}",
            "--no-hinting", "--drop-tables+=DSIG", "--desubroutinize",
            "--output-file=" + str(tmp_out),
        ]
        subprocess.run(cmd, check=True)
        out.write_bytes(tmp_out.read_bytes())


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--cjk", help="source font for Simplified Chinese")
    ap.add_argument("--arabic", help="source font for Arabic")
    ap.add_argument("--check", action="store_true",
                    help="verify the committed subsets still cover the catalogs")
    args = ap.parse_args()

    chars = catalog_chars()
    cjk, ar, rest = split(chars)

    # Всё, что не иероглиф и не арабица, обязано покрываться дефолтом egui:
    # турецкая g с бревисом или типографская кавычка у переводчика должны
    # ронять сабсет, а не уезжать квадратом к пользователю.
    base = ubuntu_light_cmap()
    if base is not None:
        missing = sorted(c for c in rest if ord(c) > 0x7F and ord(c) not in base)
        if missing:
            sys.exit("characters covered by no font: "
                     + ", ".join(f"U+{ord(c):04X} {c!r}" for c in missing))

    if args.check:
        from fontTools.ttLib import TTFont
        bad = []
        for out, need in ((CJK_OUT, cjk), (AR_OUT, ar)):
            if not out.exists():
                bad.append(f"{out.name} is missing")
                continue
            have = set(TTFont(out).getBestCmap())
            gone = sorted(c for c in need if ord(c) not in have)
            if gone:
                bad.append(f"{out.name}: " + ", ".join(f"U+{ord(c):04X}" for c in gone))
        if bad:
            sys.exit("subsets are stale, rerun without --check:\n  " + "\n  ".join(bad))
        print(f"ok: subsets cover {len(cjk)} CJK and {len(ar)} Arabic codepoints")
        return

    cjk_src, cjk_i = ((pathlib.Path(args.cjk), 0) if args.cjk
                      else pick(CJK_SOURCES, "Simplified Chinese"))
    ar_src, ar_i = ((pathlib.Path(args.arabic), 0) if args.arabic
                    else pick(AR_SOURCES, "Arabic"))

    ASSETS.mkdir(parents=True, exist_ok=True)
    subset(cjk_src, cjk_i, {ord(c) for c in cjk}, CJK_FEATURES, CJK_OUT)
    subset(ar_src, ar_i, set(range(AR_SUBSET_RANGE[0], AR_SUBSET_RANGE[1] + 1)),
           AR_FEATURES, AR_OUT)

    import fontTools
    MANIFEST.write_text(
        "Generated by scripts/subset-fonts.py — do not edit by hand.\n"
        f"fontTools {fontTools.version}\n\n"
        f"{CJK_OUT.name}\n"
        f"  source   {cjk_src} (face {cjk_i}, sha256:{sha(cjk_src)})\n"
        f"  codepoints {len(cjk)} (from i18n/*.toml)\n"
        f"  features '{CJK_FEATURES}'\n"
        f"  size     {CJK_OUT.stat().st_size} bytes, sha256:{sha(CJK_OUT)}\n\n"
        f"{AR_OUT.name}\n"
        f"  source   {ar_src} (face {ar_i}, sha256:{sha(ar_src)})\n"
        f"  codepoints U+0600..U+06FF (whole block; {len(ar)} used by the catalogs)\n"
        f"  features '{AR_FEATURES}'\n"
        f"  size     {AR_OUT.stat().st_size} bytes, sha256:{sha(AR_OUT)}\n",
        encoding="utf-8")

    print(f"{CJK_OUT.name}: {CJK_OUT.stat().st_size // 1024} KiB ({len(cjk)} codepoints)")
    print(f"{AR_OUT.name}: {AR_OUT.stat().st_size // 1024} KiB")


if __name__ == "__main__":
    main()
