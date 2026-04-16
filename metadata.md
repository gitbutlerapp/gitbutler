# Tying metadata into the editor

Currently metadata modifications sit outside of the editor. This has resulted in the `but-workspace` functions doing the bulk of the work, but leaving an often small but important detail of updating metadata to the `but-api` layer after we materialize.

From the current code-design of the `Editor` "owning" workspace interactions, having the metadata updates be considered part of that _seems_ natural. The more I think about it, it seems like a _requriement_ that we have metadata updates be considered part of the `Editor` for the following reasons:

- The encapsulation and testability of metadata updates at a `but-workspace` level.
- The `SuccessfulRebase`'s `overlayed_graph` ought to be traversed with an in-memory updated metadata.
- Any updates to `metadata` are only happening after a materialize making the operations that _should_ update metadata non-composable.
- Certain metadata like where a `WorkspaceBranch` points should be automatically updated.

## trait `RefMetadata`

The trait RefMetadata currently has one primary implementation which reads and writes from `virtual_branches.toml`.

Currently the trait sits as as both read and write access over the datastructure which
