# Fonts

Inter and Geist Mono, GitButler's UI and code fonts, from `@gitbutler/design-core` (Inter 4.001,
Geist Mono 1.401). That package ships them as variable woff2 files; these are static instances,
cut with fontTools:

```bash
fonttools ttLib.woff2 decompress Inter-Variable.woff2 -o Inter-Variable.ttf
fonttools varLib.instancer Inter-Variable.ttf wght=400 --update-name-table -o Inter-Regular.ttf
```

Each file is 60 lighter than the design-core weight it stands in for: Regular is 400 for 460,
Medium 440 for 500 and SemiBold 540 for 600. CoreGraphics, which GPUI rasterizes glyphs with,
draws the same outlines with stems about half a pixel thicker than Chrome at 2x, in both themes,
and 60 brings them level with but.dev. Weights 440 and 540 have no named instance, so those files
are cut without `--update-name-table` and keep the name table and OS/2 weight class of the 500
and 600 instances, which GPUI matches weights by.

Design-core's Inter leaves out the symbols the app's keys are written with (⌘ ⌥ ⇧ ⌃ ↩ ⌫ ⌦ ⇥, and it
has ← and → only for its `->` arrow), and without them CoreText draws those from another font,
heavier and off the baseline. Each Inter file adds them from the full Inter 4.001
(https://github.com/rsms/inter, the release design-core cut its file from), instanced at optical
size 14, as design-core's is, and at the weight the file was cut at, leaving every other glyph as
it was:

```python
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer

full = instancer.instantiateVariableFont(TTFont("InterVariable.ttf"), {"opsz": 14, "wght": 400})
base, order = TTFont("Inter-Regular.ttf"), []
for ch in "←→↩⇥⇧⌃⌘⌥⌦⌫":
    name = full.getBestCmap()[ord(ch)]
    if name not in base.getGlyphOrder() + order:
        order.append(name)
        base["glyf"].glyphs[name] = full["glyf"][name]  # none of them is a composite
        base["hmtx"].metrics[name] = full["hmtx"].metrics[name]
    for table in base["cmap"].tables:
        if table.isUnicode():
            table.cmap[ord(ch)] = name
base.setGlyphOrder(base.getGlyphOrder() + order)
base["glyf"].glyphOrder = base.getGlyphOrder()
base.save("Inter-Regular.ttf")
```

GPUI picks a weight by choosing between font files and doesn't vary a variable font, hence static
files. Both fonts are under the SIL Open Font License 1.1 (https://openfontlicense.org); each
file carries its copyright notice and license link in its name table.
