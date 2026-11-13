# Spacing

**Anything stacked in a column sits `--but-gap-stack` apart.** That covers cards
one under another, such as the sidebar's machines and repositories, or the docs
site's search, pages and appearance cards. It also covers the blocks of a modal
or a panel, such as a form over a group of settings. It is the one gap for a
stack, so every surface draws it alike: the web apps from the token, and the
GPUI client from `GAP_STACK`, which its build reads from `tokens.json`, between
its navigator's cards (`CARD_GAP`) and its tiles (`TILE_MARGIN`, half of it).
A title over its paragraph isn't a stack; its type spaces it.

**Spacing tokens name the job, not the step.** Reach for the one that says
what it spaces, as `--but-gap-buttons` or `--but-padding-card` do, and use a
step of the scale, such as `--but-space-lg`, only for room no token names yet.
When the same room shows up on a second surface, it gets a token in 🍑 But UI,
not a number in each app.
