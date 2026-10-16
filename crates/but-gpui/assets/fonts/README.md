# Fonts

Inter and Geist Mono, GitButler's UI and code fonts, from `@gitbutler/design-core` 3.17.1.
That package ships them as variable woff2 files; these are static instances at the weights the
client uses, cut with fontTools:

```bash
fonttools ttLib.woff2 decompress Inter-Variable.woff2 -o Inter-Variable.ttf
fonttools varLib.instancer Inter-Variable.ttf wght=600 --update-name-table -o Inter-SemiBold.ttf
```

GPUI picks a weight by choosing between font files and doesn't vary a variable font, hence static
files. Both fonts are under the SIL Open Font License 1.1 (https://openfontlicense.org); each
file carries its copyright notice and license link in its name table.
