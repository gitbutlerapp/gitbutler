# Links into the GitButler app

<!-- Written by `but-gpui`'s `links` tests from the app's own lists, so it's never edited by hand:
change the app, then run `SNAPSHOTS=overwrite cargo test -p but-gpui links`. -->

`but://open?path=<project>&<place>` opens the project in the directory `<project>`, percent-encoded,
at the place the parameters after it name, and without any, where the project opens by itself.
The scheme is the build's: `but`, `but-nightly` or `but-dev`.

## Places

| `to=` | A link to one |
| --- | --- |
| `project` | `to=project` |
| `section` | `to=section&section=upstream&branch=refs/heads/feature` |
| `checkout` | `to=checkout&worktree=feature` |
| `uncommitted` | `to=uncommitted&worktree=feature` |
| `branch` | `to=branch&branch=refs/heads/feature` |
| `stack` | `to=stack&branch=refs/heads/feature` |
| `commit` | `to=commit&commit=8c5a2f0e4a2b7c43c0d9a3a8bfbb4e4b2b2d1f6a&change=xrrnsmtyqworuxuywlkomlywrzkoupol` |
| `upstream` | `to=upstream&branch=refs/heads/feature&commit=8c5a2f0e4a2b7c43c0d9a3a8bfbb4e4b2b2d1f6a` |
| `mesh` | `to=mesh&checkout=other-machine&pick=branch&name=feature` |

No link opens a `terminal` or `pending`. `section=` is `upstream`, `inactive` or `mesh`, and `pick=` is `checkout`, `branch` or `commit`.

A `commit`'s `change=` is its change ID, which finds it once it's amended or rebased.

## Tabs

`tab=` opens a `project` on `overview`, `branches`, `history` or `graph`. Added to the link above to a `branch` or `upstream`,
it opens that page on `changes`, `review` or `brief`.

## Files

Added to the link above to a `section`, `uncommitted`, `branch`, `stack`, `commit`, `upstream` or `mesh`, `file=` opens its diff at a file, relative to the
project with `/`, and `line=` at the hunk with that line of its new side. With a `tab=`, only on
`changes`.

## Settings

`to=settings&page=` opens the app's settings on `general`, `appearance`, `keyboard`, `ai`, `git`, `integrations`, `experimental` or `developer`, and `to=project-settings&page=`
the project's on `project` or `git`.
