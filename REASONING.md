I decided to let the parsing fully resolve the committed hunk instead of doing a
two-stage rocket as it simplifies handling significantly. The parsing stage
already does tree diffing and on-the-fly ID computation for the tree diffs to
find committed files, so it might as well go all the way and diff blobs as well.
This tree diffing is also not cached at the moment so we can probably improve
performance here by consolidating the diffing effort.

Awkward stuff:

* The distinct split between `CommittedHunk` and `CommittedFile` variants feels
  a bit jarring when contrasted with `UncommittedHunkOrFile` variants. It's like
  this as `CommittedFile` can be resolved _without_ us even doing the blob-level
  diffs, whereas for `UncommittedHunkOrFile` both tree-level and blob-level
  diffs are always available from the `IdMap`.
* `CommittedFile` now carries a `TreeChange` which duplicates some file-related
  information (e.g. path). It needs to carry the `TreeChange` s.t. we can later
  compute the blob diff if necesseary to resolve a committed hunk
* The `IdMap` carries `diff_context_lines` which is propagated from the app
  settings in the God context object. This is a bit odd, but it makes sense as
  the IdMap is currently tasked with performing the diffing and different
  context line settings produces different diff results.
