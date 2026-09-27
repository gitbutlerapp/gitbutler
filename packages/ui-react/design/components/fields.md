# Fields

**A form field has a label.** Above the field — "Personal access token",
"Signing key", "Account email" — for every value the user has to think about:
settings, credentials, the integration and signing forms. `Field.tsx` has
`FieldLabelStyles` for the label and `FieldControlStyles` for the input.

**A name already on the surface is not given twice.** Each field needs one name
the eye and the screen reader both find. A field at the end of a settings row
is named by the row's label — "Description", "Auto-fetch frequency" — tied with
`htmlFor`; so is one under a table column heading, or in a card whose title
names its single field. `FieldLabelStyles` is for fields nothing else names, as
in forms that stack several in one strip.

**A lone field the surface already names needs no label.** A modal that asks
for one value, with a description that says which — "Enter your password for
github.com to push" — shows the field on its own; a label above it would say
"Password" a second time. The field keeps the name for screen readers as an
`aria-label`. A second field brings labels back, on every field.

**The placeholder shows the shape, when the shape needs showing.** If the user
could get the format wrong — a token, a key fingerprint, a custom API URL, a
path to a signing program — show a valid value: `GLPAT-XXXXXXXXXXXXXXXXXX`,
`723CCA3AC13CF28D`, `https://api.openai.com/v1`. An email, a name or a branch
name needs none; leave the field empty.

**A placeholder is not a label.** It vanishes once the user types and a screen
reader never has it: "Account email" as a placeholder with nothing above is a
missing label. Nothing the user needs lives only there; a token's scope, where
to generate it, what happens on save go in a hint under the field.

**An example is a value, not a caption.** A token's prefix and length, a key's
fingerprint, a full URL, a path — not the label again ("Enter your token") or a
description in words. A saved secret shows dots (`••••••••`), since its value
never comes back.

**The exceptions are fields that are the surface.** A search box, a filter row,
the command palette, a comment or reply composer: the field is the whole
control, so the placeholder does the talking — "Search for branches…", "Filter
files", "Write a reply…" — with an `aria-label` for the name. Once such a field
sits in a form with a save button, it is a form field and gets its label.
