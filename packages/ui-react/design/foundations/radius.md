# Radius

**Nested corners are concentric.** Outer radius equals inner radius plus the
padding between: a card at `--radius-card` with 4px of padding holds a control
at `--radius-card` minus 4px, not the same radius or a token picked for the
control alone. Radius tokens come from ⚛️ Core; when the subtraction doesn't
land on one, compute it with `calc()` from the outer token and say so, rather
than eyeballing a literal.
