# Git Changeset

Please note that this is a first draft, and is a little bit of a brain dump of the things that I (Caleb) have been thinking about over the last few days. I expect to come back and re-visit most of this document in the following days as we start to put together an initial implementation, and talk to some of the kind folks over at [Mercurial](https://www.mercurial-scm.org/) about their implementation.

Changeset aims to provide a way of referring to a change over time. In git, a commit is an immutable structure. "rewriting" a commit requires replacing the entire entry. That new commit does not give you any way of seeing what comes before it. Algorithms like [git-range-diff](https://git-scm.com/docs/git-range-diff) provide a best effort approach for determining what has changed, but there is an inherent imprecision, and greater cost associated with this heuristic based approach.

Changeset introduces a new inter-commit graph that can be used to traverse different revisions of a commit. Changeset gives us the ability to have a pointer to the latest revision of a commit.

This library aims to be an initial implementation of Changesets for the GitButler project, which could then be iterated on in collaboration with the kind folks from [JJ](https://github.com/martinvonz/jj) and [Sapling](https://github.com/facebook/sapling), in order to provide one standard interface which we can use. Eventually the goal is to have something like changesets introduced into git upstream, so all choices are chosen with the goal of being "git friendly".

We've chosen to initially implement changesets in Rust as its easy to integrate into other rust projects like GitButler, JJ, and Sapling. Once we've stabilized an API, and are looking to upstream into git core we might look into re-implementing in C if that is desired.

If any interesting parties has feedback or suggestions, I encourage you to reach out to me [caleb@gitbutler.com](mailto:caleb@gitbutler.com) or join our [Discord server](https://discord.com/invite/MmFkmaJ42D).

## Requirements

- A user should be able to find the latest revision of a commit for a given commit.
- A user should be able to list the different revisions for a given changeset.
- Changesets should be able to be worked on by two different users remotely, with ways of merging together two diverged versions of a particular changeset.
- Changesets should have a unique identifier that tools can use to refer to them by.
- Refer to an arbitrary commit by a changeset, without rewriting it.
- Changesets need to be easily GCed.

## Data structures

### Changeset (v0.1)

A changeset stores the OIDs of every edge in the inter-commit graph, as well as a pointer to the head.

A changeset is another type of special commit.
A changeset is assigned a randomly generated 160bit identifier which is encoded as reverse hexadecimal. By reverse decimal, we mean that `0 to f` is mapped into `z to k`. For example, `01ef` in reverse hexadecimal would be `zylk`. The use of this reverse hexadecimal allows us to easily differentiate between a regular object OID and a changeset ID.

```
Headers:
- changeset-version: Used to indicate the version of the changeset schema used.
- changeset-id: The changeset's own ID.
- changeset-head: The OID of the head Edge

Tree:
- [first two OID chars] / [rest OID chars]: Points to the commit of an edge. Entry is of type commit.
```


### Edge (v0.1)

An `Edge` is a special type of commit which stores information about the transition from one state to another.

```
Headers:
- edge-version: Used to indicate the version of the edge data schema used.
- parent: May be provided multiple times, and refers to edge IDs which are the immediate parents of this edge.
- edge-commit: Refers to the commit at the new node. The commit is optionally present in the ODB. A user can push a sparse inter-commit graph as intermediary changes may be uninteresting or even contain information a user does not want to share.
- edge-from: May be provided multiple times. The changeset-id for which the edge has come from.
- edge-to: The changeset-id that the edge is going to.

Tree:
- An empty tree

Message:
- An empty message
```

#### Internal Edge

An internal edge is an edge which shows the changeset going from one revision to another.

An internal edge's `edge-from` and `edge-to` headers will be the same.

##### Internal Merge Edge

An internal merge edge is analogous to a merge commit. It represents that there were two diverged versions of a changeset which have now been merged back together.

#### Incoming Edge

An incoming edge has multiple parent edges, where one is from another changeset's graph. An incoming edge will also have an `edge-from` header for each of the changesets that are getting incorporated into this change.

Read the Squashing and Splitting sections for which sides should maintain their Change IDs.

An incoming edge must be recorded in the edge-trees of each changeset recorded in the operation.

#### Root Edge

A root edge is used to refer to the first commit in a changeset

## Indexes / References

The `Changeset` and `Edge` objects enable us to create a DAG of changes, and keep track of them to some extent, but on their own don't provide us a way of finding a changeset for a given commit, or even from stopping commits from getting GCed in the first place.

As such we need to employ the use of some indexes. We however acknowledge that indexes might not be a one-size fits all solution, so the indexes are going to be plug-able.

### `.gitchangesets` Configuration

There will be a committable `.gitchangesets` file. If the file is not present, it will be assumed that changesets are currently not in use, though the file may be created if any of the changeset features are used, in which case the `trees` backend will be assumed.

The `.gitchangesets` file uses syntax that strictly should be compatible with both TOML and "git config" syntax.

The git changesets file has two properties:
- `changesets.version`: to indicate version (currently 0)
  - The version indicates that all data structures will be of version 0.x
- `changesets.backend`: which may indicate (currently either "disabled" or "trees").
  - If the backend is set to "disabled", running any changesets command will return an error.

### Trees backend

The trees backend contains two indexes. One which indexes the changesets, and another which contains the mappings from commits to changesets.

The trees backend is designed such that multiple changeset-enabled clients can share changesets, but without requiring any first-party forge support.

#### Changesets index (v0.1)

The changesets index is a special type of Commit which contains a special tree. The changesets index commit will be referenced by `refs/changesets/changeset-index`.

```
Headers:
changeset-index-version: Used to indicate the version of the changeset index data schema used.
parent: No parents should be provided.
```

```
Tree:
    tree changesets: Contains an entry named `<first 2 changeset ID chars>/<rest of changeset ID chars>` and has the entry value as a commit ID. The reason we have this is to prevent the changeset ID from getting GCed by git prematurely.
```

##### GC

Entries can be removed from the index when the changeset is no longer referenced by any branch.

#### Commit to changeset index (v0.1)

The commit to changeset index is a special type of Commit which contains a special tree. The changesets index commit will be referenced by `refs/changesets/commit-to-changeset-index`.

```
Headers:
commit-to-changeset-index-version: Used to indicate the version of the commit to changeset index data schema used.
parent: No parents should be provided.
```

```
Tree:
    tree changesets: Contains an entry named `<first 2 commit ID chars>/<rest of commit ID chars>/<changeset ID>` and the value is the commit itself.
```

##### GC

Entries can be removed from the index when the changeset they point to is no longer present.

## Commands

The following list contains some basic primitives which will be used to explain how a git client might interact with changesets. These will likely start off as functions which require callbacks in order to perform networking, 

- `changeset upsert [--parent <edge-oid>] [--from <edge-oid>] [--to <edge-oid>] [<changeset-id>] <commit-oid>`
  - Adds a new entry into the changeset database.
  - If a changeset-id is provided, that changeset will be updated, otherwise a new changeset will be created.
- `changeset push [--force] <remote> <changeset-id> [<changeset-id>]`
  - Pushes an individual changeset to a remote. By default it will push the local changeset to a matching remote changeset-name, unless otherwise specified.
  - If the changeset is diverged, the push will be aborted unless the `--force` flag is provided.
- `changeset merge <remote> <changeset-id> [<changeset-id>]`
  - Merges a remote changeset into a local one. By default it will merge into the local changeset with the same name, unless otherwise specified.
  - After merging, if that changeset was referred to by a branch, that branch will require rebasing such that it includes the resulting commit.

  - If there is a conflict, a resolution will be requested.

  - Returns the resulting changeset commit, edge commits, and resulting commit.
- `changeset reset <remote> <changeset-id> [<changeset-id>]`
  - Resets a local changeset to match a local one. By default it will merge into the local changeset with the same name, unless otherwise specified.
  - After resetting, if that changeset was referred to by a branch, that branch will require rebasing such that it includes the resulting commit.

  - Returns the resulting changeset commit, edge commits, and resulting commit.
- `changeset show <changeset-id | commit-id>`
  - If a changeset-id is provided, it will return an overview of the changeset
  - If a commit-id is provided, it will return an overview of the commit's changeset
- `changeset branch status [<ref-name | commit-id>]`
  - Returns information about any changesets that may belong to the commits in the current branch, or in the specified branch.
- `changeset branch push [--force] [<ref-name | commit-id>]`
  - Pushes the changesets associated with a branch
  - If the a changeset is diverged, the push will be aborted unless the `--force` flag is provided.

