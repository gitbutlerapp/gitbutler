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

GPUI picks a weight by choosing between font files and doesn't vary a variable font, hence static
files. Both fonts are under the SIL Open Font License 1.1 (https://openfontlicense.org); each
file carries its copyright notice and license link in its name table.
