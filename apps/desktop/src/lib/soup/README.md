# Soup… Who Dis?

In our never-ending quest for software development enlightenment, we’ve stumbled
upon yet another existential question:

- Where do we put classes whose sole purpose is to combine other services for
  frontend use?

These "soup classes" mix together various behaviors from existing services into
a single, convenient interface—just enough for the view layer to slurp up.

> Rule of thumb: Soup classes should only be consumed by view components.

Worried this pattern is a code smell? Don't be. This kind of composition isn't a
weakness—it's a soup-er power.
