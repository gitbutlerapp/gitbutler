# Graph Creation

This is unfortunatly a difficult problem because:
- We want to try and pull from the `but-graph` to try and generate a graph which
is as "semantically sensible" as possible.
- We can't trust the `but-graph`. The but graph:
    - Can have out-of-order parents
    - Can have missing parents
    - Can have duplicated parents

In all but the case of the workspace commit, it is imparative that we get the
commit parantage correct.

There is one exepction which is the workspace commit. We should do whatever the
`but-graph` tells us. This _might_ result in extra rebases, but that is OK since
it is a managed commit.

