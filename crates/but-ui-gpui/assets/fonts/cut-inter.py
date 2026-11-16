"""Cut the static Inter files GPUI draws with from but-ui's variable Inter, each 60 lighter than
the but-ui weight token it stands in for, as README.md says why.

    python3 -I cut-inter.py path/to/InterVariable.ttf

The argument is the full Inter 4 variable font (rsms/inter, docs/font-files/InterVariable.ttf at
the v4.0 tag), which the key symbols come from. It needs fontTools (`pip install fonttools
brotli`). Run it after a `--but-font-weight-*` token changes and commit the files it writes;
`fonts::tests` fails until they match the tokens.
"""

import copy
import re
import sys
from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables import otTables
from fontTools.varLib import instancer

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
VARIABLE = ROOT / "packages/but-ui/fonts/inter/Inter-Variable.woff2"
TOKENS = ROOT / "packages/but-ui/tokens/tokens.css"

# How much lighter a file is cut than the weight it stands in for: CoreGraphics draws Inter's
# stems heavier than Chrome, and 60 brings them level with but.dev (README.md).
OFFSET = 60
# The key symbols but-ui's Inter leaves out, taken from the full Inter at the same weight.
SYMBOLS = "←→↩⇥⇧⌃⌘⌥⌦⌫"
# The record each file says what it was cut for in, which the test reads.
DESCRIPTION = 10


def token(name: str) -> int:
    found = re.search(rf"--but-font-weight-{name}:\s*(\d+)", TOKENS.read_text())
    if not found:
        sys.exit(f"--but-font-weight-{name} isn't in {TOKENS}")
    return int(found.group(1))


# Each file: the weight it stands in for and what that weight is called, then the named instance
# whose names, OS/2 weight class and STAT weight it carries, as GPUI and DirectWrite match a
# weight by those, and the name record STAT calls that weight.
FILES = [
    ("Inter-Regular.ttf", token("regular"), "--but-font-weight-regular", 400, 2),
    ("Inter-Medium.ttf", 500, "500, which but-ui has no token for", 500, 307),
    ("Inter-SemiBold.ttf", token("semibold"), "--but-font-weight-semibold", 600, 308),
]


def cut(variable: TTFont, full: TTFont, file: str, weight: int, called: str, named: int, name_id: int) -> None:
    at = weight - OFFSET
    font = instancer.instantiateVariableFont(copy.deepcopy(variable), {"wght": at})
    identity = instancer.instantiateVariableFont(
        copy.deepcopy(variable), {"wght": named}, updateFontNames=True
    )
    font["name"] = identity["name"]
    font["STAT"] = identity["STAT"]
    font["OS/2"].usWeightClass = identity["OS/2"].usWeightClass
    font["OS/2"].fsSelection = identity["OS/2"].fsSelection
    font["head"].macStyle = identity["head"].macStyle

    # The instancer drops the weight from STAT, which DirectWrite reads a face's weight from:
    # without it Windows drew Medium as Regular and SemiBold as Regular made bold (README.md).
    stat = font["STAT"].table
    wght = next(i for i, axis in enumerate(stat.DesignAxisRecord.Axis) if axis.AxisTag == "wght")
    values = stat.AxisValueArray.AxisValue
    if not any(v.AxisIndex == wght and getattr(v, "Value", None) == named for v in values):
        value = otTables.AxisValue()
        value.Format, value.AxisIndex, value.Flags = 1, wght, 0
        value.ValueNameID, value.Value = name_id, named
        values.insert(wght, value)
        stat.AxisValueCount = len(values)

    symbols = instancer.instantiateVariableFont(copy.deepcopy(full), {"opsz": 14, "wght": at})
    order, added = font.getGlyphOrder(), []
    for ch in SYMBOLS:
        glyph = symbols.getBestCmap()[ord(ch)]
        if glyph not in order + added:
            added.append(glyph)
            font["glyf"].glyphs[glyph] = symbols["glyf"][glyph]  # none of them is a composite
            font["hmtx"].metrics[glyph] = symbols["hmtx"].metrics[glyph]
        for table in font["cmap"].tables:
            if table.isUnicode():
                table.cmap[ord(ch)] = glyph
    font.setGlyphOrder(order + added)
    font["glyf"].glyphOrder = font.getGlyphOrder()

    font["name"].setName(f"Cut at wght {at} for {called}, {weight}", DESCRIPTION, 3, 1, 0x409)
    # Plain TrueType, not the woff2 the source comes in.
    font.flavor = None
    font.recalcTimestamp = False
    font.save(HERE / file)
    print(f"{file}: wght {at}, for {called} ({weight})")


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    variable = TTFont(VARIABLE)
    full = TTFont(sys.argv[1])
    for file, weight, called, named, name_id in FILES:
        cut(variable, full, file, weight, called, named, name_id)


main()
