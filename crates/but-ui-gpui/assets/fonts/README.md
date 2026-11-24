# Fonts

Inter, GitButler's UI font, from `@gitbutler/but-ui` (Inter 4.001). That package ships it as
a variable woff2 file. GPUI keeps a font added as bytes at its default instance, so it never
reaches a variable file's other weights: these are static instances of it, which
`cut-inter.py` cuts from but-ui's weight tokens. After a `--but-font-weight-*` token changes, run
it and commit the files it writes; `fonts::tests` fails until they match the tokens.

```bash
pip install fonttools brotli
python3 -I cut-inter.py path/to/InterVariable.ttf
```

The argument is the full Inter 4 variable font, `docs/font-files/InterVariable.ttf` at
[rsms/inter](https://github.com/rsms/inter)'s `v4.0` tag, which the key symbols come from.

Each file is 60 lighter than the but-ui weight it stands in for: Regular is 400 for
`--but-font-weight-regular`, 460, and SemiBold 520 for `--but-font-weight-semibold`, 580. Medium,
440, stands in for a 500 but-ui doesn't have; only Markdown's italics and the graph's labels draw
with it. Each file says what it was cut for in its Description name record, which the test reads.
CoreGraphics, which GPUI rasterizes glyphs with, draws the same outlines with stems about half a
pixel thicker than Chrome at 2x, in both themes, and 60 brought them level with but.dev when the
client turned macOS font smoothing off, which it no longer does, so text now reads heavier there.
Compare against but.dev before changing the offset: render both at the same scale (headless
Chrome with `--force-device-scale-factor=2`, and a `drive.sh` `shot` of a region, which keeps the
display's scale) and compare stem thickness, not screenshots taken at different scales.

A weight between named instances has no names of its own, so each file carries the name table,
OS/2 weight class and `STAT` weight of the named instance it stands for (Regular 400, Medium 500,
SemiBold 600), which GPUI matches weights by; the text classes pick a file by them (`text.rs`).
The instancer drops the weight value from `STAT`, which DirectWrite reads a font's weight from.
Without it Windows read Medium and SemiBold as weight 1, drew Medium as Regular, and drew
SemiBold as Regular made bold, in a new font face each time. GPUI on Windows remembers faces by
their address, so once Windows reused one for a Chinese, Japanese or Korean font, that text drew
in Inter, as boxes. The script puts the value back.

Design-core's Inter leaves out the symbols the app's keys are written with (⌘ ⌥ ⇧ ⌃ ↩ ⌫ ⌦ ⇥, and it
has ← and → only for its `->` arrow), and without them CoreText draws those from another font,
heavier and off the baseline. The script adds them from the full Inter, instanced at optical size
14, as but-ui's is, and at the weight the file is cut at, leaving every other glyph as it was.

Inter is under the SIL Open Font License 1.1 (https://openfontlicense.org); each
file carries its copyright notice and license link in its name table.

## Code and terminal font

Code and terminals draw with Geist Mono Nerd Font Mono (`GeistMonoNerdFontMono-*.otf`), Geist Mono
1.702 patched by Nerd Fonts v3.5.1, unmodified:

```bash
gh release download v3.5.1 -R ryanoasis/nerd-fonts -p GeistMono.tar.xz
tar xf GeistMono.tar.xz GeistMonoNerdFontMono-Regular.otf GeistMonoNerdFontMono-SemiBold.otf LICENSE
```

The Geist Mono in but-ui is a subset. It leaves out most accented Latin letters, as 16 of
Polish's 18, and the box-drawing characters and Private Use Area file icons TUIs draw with.
CoreText takes those from other fonts with other designs and widths, so words mix two fonts,
rows drift off the grid, borders gap, and icons show as missing glyphs. The Mono variant keeps
every glyph, icons included, one cell wide. Nerd Fonts ships only these fixed weights, so bold is
600 rather than 540, and it has no ligatures, so the Font ligatures setting shows only in a font
the user picks. CoreText names the family "GeistMono Nerd Font Mono", not the "GeistMono NFM" in
the files' legacy names.

Nerd Fonts' patched fonts are under the SIL Open Font License 1.1
([licensing](https://github.com/ryanoasis/nerd-fonts/blob/v3.5.1/LICENSE)), and
`LICENSE-GeistMonoNerdFont.txt` is the license from the release archive.
