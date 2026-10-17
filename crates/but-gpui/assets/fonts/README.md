# Fonts

Inter and Geist Mono, GitButler's UI and code fonts, from `@gitbutler/design-core` (3.17.1, and
10.0.0 for the Regular files). That package ships them as variable woff2 files; these are static
instances at the weights the client uses, cut with fontTools:

```bash
fonttools ttLib.woff2 decompress Inter-Variable.woff2 -o Inter-Variable.ttf
fonttools varLib.instancer Inter-Variable.ttf wght=600 --update-name-table -o Inter-SemiBold.ttf
```

Regular is design-core's regular weight, 460, rather than 400. The variable fonts name no instance
at 460, so those two are cut without `--update-name-table`, and keep the name table of the 400
instance they replaced, with the OS/2 weight class set to 460.

GPUI picks a weight by choosing between font files and doesn't vary a variable font, hence static
files. Both fonts are under the SIL Open Font License 1.1 (https://openfontlicense.org); each
file carries its copyright notice and license link in its name table.
