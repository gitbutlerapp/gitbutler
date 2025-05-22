#![allow(unused_variables)]
//! What follows is a description of all entities we need to implement Workspaces,
//! a set of *two or more* branches that are merged together.
//!
//! ## tl/dr
//!
//! - `Switch` is a way to checkout any branch
//!
//! ## Rules
//!
//! The following rules and values govern how certain operations work. Ideally, everything that is written here can be
//! reproduced by extrapolating them.
//!
//! * `HEAD` always points to a workspace.
//!     - **Implicit Workspace**
//!          - a commit (zero, one or more parents) created by the user, possibly pointed to by a branch.
//!     - **Implicit Workspace with Metadata**
//!          - like above, but has a `gitbutler/workspace/name` ref pointing to it which typically carries *ref-metadata*
//!     - **Workspace with Metadata**
//!          - a *workspace commit* with `gitbutler/workspace/name` reference pointing to it.
//!          - always has two or more parents, otherwise it has no reason to exist.
//! * _Optional_ *Target branches* are implied, globally explicit or don't exist at all
//!     - If they exist, allow computing the highest/most-recent *target merge-base* that *stacks* in the
//!       *workspace* are forked-off from.
//!     - The *target merge-base* informs about how many commits we are behind *or* ahead.
//!     - The whole workspace can be made to include more recent (or older) versions of the *target branch*
//!     - A *target branch* is typically initialized from the remote tracking branch of the current branch at `HEAD`.
//!     - Alternatively it's configured for the whole project.
//!  * *Target merge bases* _should_ not change implicitly
//!     - This merge-base is the 'view of the world' for the user as it's the *most recent merge-base with the target branch available in the worktree*.
//!       Hence, it should remain stable.
//!     - Adding branches to the workspace with *their* *target merge base* in our *past* is OK.
//!     - Adding branches to the workspace with *their* *target merge base* in our *future* is not OK
//!       unless the user allows it, or…
//!        - We may try to *rebase* branches from the future onto our *target merge base*
//!          if they have no remote tracking branch.
//!  * *Workspace Merge Base*
//!     - Is automatically the *Target Merge Base* if there is just a single branch.
//!     - Is the octopus merge-base between all stacks in a workspace, i.e. a commit reachable from the tips of all stacks.
//!  * *Remote Tracking Branches* are considered to integrate stacks with
//!     - The *Target branch* can be the remote tracking branch of a local tracking branch that is part of the workspace.
//!  * *Stacks* and their commits
//!     - Are listed in such a way that they don't repeat across multiple stacks, i.e. only commits that aren't in
//!      any of the other stacks or in the target branch (if present).
//!  * *Stack Segment* - a set of connected commits between reference name and another reference name, or the lower bound of the stack.
//!
//! TODO:
//!  - About reference points for rev-walks (WMB, TMB, auto-target when branch has PR with target to merge into)
//!    Without reference points, walks will be indefinite.
//!
//! ## Operations
//!
//! * **add branch from workspace**
//!    - Add a branch to a *workspace commit* so it also includes the tip of the branch.
//! * **remove branch from workspace**
//!    - Remove a branch from a *workspace commit*
//!    - Alternatively, checkout the *target branch*
//! * **switch**
//!    - Switch between branches of any kind, similar to `git switch`, but with automatic worktree stashing.
//! * **stash changes**
//!    - Implicit, causes worktree changes to be raised into their own tree and commit on top of `HEAD`.
//! * **apply stashed changes**
//!    - Lower previously stashed changes by re-applying them to a possibly changed tree, and checking them
//!      out to the working tree.
//!
//! ## Metadata
//!
//! Metadata is all data that doesn't refer to Git objects (i.e. anything with a SHA1), and can be associated with
//! a full reference name.
//!
//! ## Target Branch
//!
//! The branch that every branch in a workspace wants to get integrated with,
//! either by merging or by being rebased onto it.
//!
//! The target branch is always a *remote tracking branch*, and as such it's not expected to be checked out
//! nor would it be committed into directly.
//!
//! ## Rebase Considerations
//!
//! We avoid rebasing when adding something to the workspace,
//! either because the added stack as a remote tracking branch, or because its merge-base with the target branch
//! is in the past.
//!
//! If we were to *rebase* as part of a workspace update or adding to the stack, we would have to try to assure that the user isn't
//! *accidentally pushing auto-resolved conflicting commits* that this might generate.
//!
//! This could be achieved by temporarily removing branch remote configuration (literally), so it can
//! be put back once it's safe to push. Alternatively, and additionally, the ref could just be put back to
//! its previous location.
//!
//! Alternatively, auto-resolves could be deactivated if we are in Git mode, and instead they are applied
//! to the worktree directly, and we wait (with sequencer support) until the conflict has been resolved.
//!
//! ## Workspace Tip
//!
//! This is the elaborate name of the commit that currently represents what's visible in the working tree,
//! i.e. the commit that `HEAD` points to.
//!
//! `HEAD` can point to Git commits, and to *Workspace Commits*, and the user switches `HEAD` locations
//! at will with any tool of choice.
//!
//! ## Workspace Commits
//!
//! Whenever there are more than one branch to show, a *workspace commit* is created to keep track of the branches
//! that are contained in it, making it *always a merge commit*.
//!
//! ### Workspace References
//!
//! Whenever a workspace is officially created by means of a *workspace commit*, a `refs/heads/gitbutler/workspace/<name>`
//! is also created to point to it.
//! These are removed once the corresponding *workspace commit* is going out of scope and if they don't contain any
//! ref-metadata worth holding on to.
//!
//! *This also makes workspaces enumerable*.
//!
//! ## Worktree Stashes via Stash Commits
//!
//! Whenever there are worktree changes present before switching `HEAD` to another location,
//! we will leave them if they don't interfere with the new `HEAD^{tree}` that we are about switch to, as a `safe` checkout
//! is assumed.
//! Those that *do interfere* we pick up and place as special *stash commit* on top of the commit
//! that can accommodate them and let a *stash reference* point to it.
//!
//! ### Stash References
//!
//! The *stash reference* is created in the *gitbutler-stashes* [Git namespace](gix::refs::file::Store::namespace) to
//! fully isolate them from the original reference name. This also means that we should disallow renaming branches with
//! an associated *stash reference* and make it possible to list orphaned stashes as well, along with recovery options.
//!
//! For instance, if a references is `refs/heads/gitbutler/workspace/one`, then the stash reference is in
//! `<namespace: gitbutler-stashes>/refs/heads/gitbutler/workspace/one`. The reason for this is that they will not
//! be garbage-collected that way.
//! Note that any ref, like `refs/heads/feature` can carry a stash, so *workspace references* aren't special.
//!
//! *This also makes stashes enumerable*. It's notable that it's entirely possible for stashes to become *orphaned*,
//! i.e. their workspace commit (that the stash commit sits on top of) doesn't have a reference *with the same name*
//! pointing to it anymore. What's great is that these can easily be recovered, along with the stash
//! as it's trivial to find the *workspace commit* as only parent of the *stash commit*.
//!
//! They can also be *desynced*, meaning that the parent reference doesn't point to the parent of the *stash commit* anymore.
//!
//! ### Raising a Stash
//!
//! This is the process of knowing what to put in a stash in the first place before changing the worktree.
//! The idea is to *minimize* the stash and leave everything that isn't going to be touched by the final checkout in place.
//! That way what's in the stash is only the files that need to be merged back, minimizing the chance for conflicts.
//!
//! ### Apply Stashed Changes/Lowering a Stash
//!
//! Stashed changes are cherry-picked onto the target tree, but *merge conflicts* have to be kept and checked out
//! to manifest them, similar to what happens when a Git stash is popped.
//!
//! If the destination of an unapply operation already has a stash, both source and destination stash will be merged
//! with merge-conflicts manifesting in the tree.
//!
//! ### Shortcomings
//!
//! * **changed indices prevent stash creation**
//!     - If changes in the .git/index would be ignored, data stored only there would be lost.
//!     - conflicts would be lost, and should probably prevent the stash in any case.
//! * **complexities with multiple worktree stashes**
//!     - The user can leave a stash in each worktree that they switch away from.
//!     - When switching back to a commit with stash we now have to deal with two stashes, the one that was already
//!       there and the one we newly created before switching. Depending on the operation we may have to merge both
//!       stashes, which can conflict.
//!
//! ## Sketches
//!
//! ```text
//!  ┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
//!  │  H: Head | S: Stack | T: Target | GT: Global Target | RTB: Remote Tracking Branch | TMB: Target Merge Base | WTC: Worktree Changes   │
//!  ├──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
//!  │                                                      WMB: Workspace Merge Base                                                       │
//!  └──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
//!
//!           ████████████████████ Apply feat ██████████████████████████████████████  Unapply main ████████████████████████████
//! ┌───┐
//! │   │
//! │   │                                                                                                       - WARN: TMB changed
//! │   │                                                                                                       - keep T info in ws/1
//! │ C │                                                     ┌──┐
//! │ a │                                                     │WS│◀── ws/1
//! │ u │    T:RTB/main                 T:RTB/main ──┐        └──┘                               T:RTB/main
//! │ g │         │                                  │          ▲                                     │
//! │ h │         │                                  │   ┌──────┴─────┐                               │            ┌────H:ws/1
//! │ t │         ▼  H:S:main                        │   │            │                               ▼            ▼
//! │   │        ┌─┐     │    ┌─┐                    │  ┌─┐          ┌─┐                             ┌─┐          ┌─┐
//! │ U │        TMB◀────┘    └─┘◀── feat            └─▶TMB◀─S:foo   └─┘◀┬─S:feat                    └─┘◀─ foo    └─┘◀─S:feat⇕⇡1⇣1
//! │ p │         │            │                         │            │  │                            │            │
//! │   │         │            │                         │            │  │                            │            │
//! │   │         │            │                         │           RTB/feat                         │            │
//! │   │        ┌─┐           │                        ┌─┐           │                              ┌─┐           │
//! │   │        WMB───────────┘                        WMB───────────┘                              WMB───────────┘
//! └───┘                                                                                            TMB
//!
//!
//!           ████████████████████ Apply feat ██████████████████████████████████████  Unapply main ████████████████████████████
//! ┌───┐
//! │   │
//! │   │
//! │ N │      WMB missing                                    ┌──┐                             MB missing
//! │ o │      - show all reachable commits                   │WS│◀── ws/1                     - show all reachable commits
//! │   │                                                     └──┘
//! │ T │                                                       ▲
//! │ a │                                                ┌──────┴─────┐
//! │ r │            H:S:main                            │            │
//! │ g │        ┌─┐     │    ┌─┐                       ┌─┐          ┌─┐                             ┌─┐          ┌─┐
//! │ e │        └─┘◀────┘    └─┘◀── feat               └─┘◀─S:main  └─┘◀──S:feat                    └─┘◀─ main   └─┘◀─H:S:feat
//! │ t │         │            │                         │            │                               │            │
//! │   │         │            │                         │            │                               │            │
//! │   │         │            │                         │            │                               │            │
//! │   │        ┌─┐           │                        ┌─┐           │                              ┌─┐           │
//! └───┘        └─┘───────────┘                        WMB───────────┘                              └─┘───────────┘
//!
//!
//!
//!           ████████████████████ Apply feat ██████████████████████████████████████████████ FF main in Git ██████████████████████ Unapply feat ████████
//! ┌───┐
//! │   │    GT:RTB/main
//! │ T │         │                                           ┌──┐                S:main ─┐       ┌──┐                  H:S:main─┐        - It goes back to the ref
//! │ a │         ▼                                     ┌─┐   │WS│◀──H:ws/1               │ ┌─┐   │WS│◀──H:ws/1                  │ ┌─┐    - TMB moved
//! │ r │        ┌─┐                       GT:RTB/main─▶└─┘   └──┘            GT:RTB/main─┴▶└─┘   └──┘               GT:RTB/main─┴▶TMB
//! │ g │        └─┘                                     │      ▲                            │      ▲                               │           ┌─┐
//! │ e │         │                                      ├──────┴─────┐                      ├──────┴─────┐                         │           └─┘◀── feat
//! │ t │         │  H:S:main⇕⇣1             S:main⇕⇣1   │            │                      │            │                         │            │
//! │   │        ┌─┐      │   ┌─┐                │      ┌─┐          ┌─┐                    ┌─┐          ┌─┐                       ┌─┐           │
//! │ A │        TMB◀─────┘   └─┘◀── feat        └─────▶TMB          └─┘◀──S:feat           TMB          └─┘◀──S:feat              └─┘           │
//! │ h │         │            │                         │            │                      │            │                         │            │
//! │ e │         │            │                         │            │                      │            │                         │            │
//! │ a │         │            │                         │            │                      │            │                         │            │
//! │ d │        ┌─┐           │                        ┌─┐           │                     ┌─┐           │                        ┌─┐           │
//! │   │        └─┘───────────┘                        WMB───────────┘                     WMB───────────┘                        └─┘───────────┘
//! └───┘
//!
//! ┌───┐
//! │ W │     ███████████ Create Commit ██████████████ Push to remote ██████████ Create commit ████████████ Push to remote ████████████
//! │ o │
//! │ r │
//! │ k │
//! │   │
//! │ o │                                                                                   ┌─┐        T:RTB/main ──┐  ┌─┐
//! │ n │                                                                  H:S:main⇕⇡1 ────▶└─┘          H:S:main ──┴─▶└─┘
//! │   │                                                                                    │                         WMB
//! │ T │                                                                                    │                         TMB
//! │ a │                                               T:RTB/main─┐                         │                          │
//! │ r │                                        ┌─┐               │  ┌─┐                   ┌─┐                        ┌─┐
//! │ g │          H:S:main         H:S:main────▶└─┘      H:S:main─┴─▶└─┘ T:RTB/main⇕⇣1 ───▶└─┘                        └─┘
//! │ e │                                                                                   WMB
//! │ t │                                                                                   TMB
//! │   │
//! └───┘                                                                  Target branch is behind
//!
//!
//!
//! ┌───┐     ███████████ Create Commit ██████████████Create Stack █████████████Create Branch █████████████████████ Commit feat/1 ████████████████████ Insert Commit to feat ██████████████████████ Commit main ████████████████████████████████ Push main █████████████████████████████████ Push S:feat/1 and B:feat ████████████████ Merge feat/1 PR and fetch ████████████████████████████ Update Workspace █████████████████████████████ Prune Integrated ██████████████████████████
//! │   │
//! │ F │                                                                                                                    ┌──┐                                ┌──┐                                    ┌──┐                                      ┌──┐                                         ┌──┐                                             H:ws/1
//! │ r │                                                                                                          H:ws/1 ──▶│WS│────┐                 H:ws/1 ──▶│WS│───┐   ┌─┐                H:ws/1 ──▶│WS│───┐   ┌─┐                  H:ws/1 ──▶│WS│───┐   ┌─┐                     H:ws/1 ──▶│WS│───┐   ┌─┐ ┌── RTB/feat/1                 ┌─┐   │                                                ┌──┐
//! │ e │                                                                                                                    └──┘    │                           └──┘   └───└─┘◀─── S:feat/1             └──┘   └───└─┘◀─── S:feat/1               └──┘   └───└─┘◀─── S:feat/1                  └──┘   └───└─┘◀┴── S:feat/1    T:RTB/main ───▶└─┘◀──┼─────────┐                                 ┌────│WS│──────┐
//! │ s │                                                            ┌──┐                       ┌──┐              WS aids      │    ┌─┐                            │         │                             │         │                               │         │                                  │         │                                  │    ▼         │                                 │    └──┘      │
//! │ h │                                                 H:ws/1 ───▶│WS│            H:ws/1 ───▶│WS│              traversal by │    └─┘◀─── S:feat/1               │        ┌┴┐                       ┌────┘        ┌┴┐                         ┌────┘        ┌┴┐                            ┌────┘        ┌┴┐ ┌──  RTB/feat                   │  ┌──┐        │                 T:RTB/main ─┐  ┌─┐     ▲       │                T:RTB/main ─┐  ┌─┐
//! │   │                                                            └──┘                       └──┘              parent       │     │                             ├────────┴─┘◀─── B:feat            │             └┬┘◀─── B:feat              │             └┬┘◀─── B:feat                 │             └┬┘◀┴── B:feat                      │  │WS│───┐   ┌─┐ ┌── RTB/feat/1     S:main ─┴─▶TMB─────┼───────┤                   H:S:main─┴─▶└─┘─────────────┐
//! │ I │                                                              │                          │                            ├─────┘                             │                                 ┌─┐             │                         ┌─┐             │                            ┌─┐             │                                  │  └──┘   └───└─┘◀┴── S:feat/1                   │      │       │                               WMB             │
//! │ n │                                                              │                          │                            │                                   │                       S:main ──▶└─┘──┐          │              S:main ─┬─▶└─┘──┐          │                 S:main ─┬─▶└─┘──┐          │                                  │    │         │                                 │   H:ws/1    ┌─┐ ┌── RTB/feat/1               TMB            ┌─┐
//! │ i │                                        ┌─┐                  ┌─┐                        ┌─┐ ┌─ S:feat/1              ┌─┐                                 ┌─┐                                     └─┬─┬──────┘          T:RTB/main ─┘       └─┬─┬──────┘             T:RTB/main ─┘       └─┬─┬──────┘                                  ├────┘        ┌┴┐ ┌──  RTB/feat                  │             └─┘◀┴── S:feat/1                  │             └─┘
//! │ t │          H:S:main         H:S:main────▶└─┘      S:main ────▶└─┘◀── S:feat    S:main ──▶└─┘◀┴─ B:feat      S:main ──▶WMB◀───── B:feat          S:main ──▶WMB                                       WMB                                       └─┘                                          └─┘                                         │             └┬┘◀┴── B:feat                     │              │                                │              │
//! │   │                                                                                                                                                                                                                                             WMB                                          WMB                                        ┌─┐             │                                 │             ┌┴┐ ┌──  RTB/feat                 │             ┌┴┐
//! │   │                                                                                                                                                                                                                                             TMB                                          TMB                            S:main⇕⇣───▶└─┘──┐          │                                 │             └┬┘◀┴── B:feat                    │             └┬┘
//! └───┘                                                 Need ws/1 for metadata:      - Need ws/1 for metadata:                                                                                                                                                                                                                                   └─┬─┬──────┘                                ┌─┐             │                               ┌─┐             │
//!                                                       stack order                    stack + branch order                                                                                                                                                                                                                                        └─┘                                       └─┘──┐          │                               └─┘──┐          │
//!                                                                                    - Stack name changed to                                                                                                                                                                                                                                       WMB                                            └─┬─┬──────┘                                    └─┬─┬──────┘
//!                                                                                      feat/1                                                                                                                                                                                                                                                      TMB                                              WMB                                             └─┘
//!
//!
//!
//!  ┌───┐     ████████████████████ switch to feat and change WT ██████████████████ switch to main █████████████████████████████                                                                                                                                                                                                                                                   - FF S:main                                     - delete all of S:feat/1 as we created it as well.
//!  │   │                                                       ┌─┐                                                   ┌─┐                                                                                                                                                                                                                                                         - Detect S:feat/1 and B:feat are merged and     - delete ws/1 as its metadata can be inferred
//!  │ A │                                            WTC/main ─▶└─┘                                        WTC/feat ─▶└─┘                                                                                                                                                                                                                                                           do nothing.
//!  │ u │                                                        │                                                     │                                                                                                                                                                                                                                                          - remerge WS as S:main tip changed
//!  │ t │                                                        │                                                     │
//!  │ o │                ┌─┐      ┌─┐                           ┌─┐      ┌─┐                                 ┌─┐      ┌─┐
//!  │ S │ H:S:WTC:main ─▶└─┘      └─┘◀── feat            main ─▶└─┘      └─┘◀── H:S:WTC:feat              ┌─▶└─┘      └─┘◀── feat
//!  │ t │                 │        │                             │        │                               │   │        │
//!  │ a │                 │        │                             │        │                 H:S:WTC:main ─┘   │        │
//!  │ s │                 │        │                             │        │                                   │        │
//!  │ h │                ┌─┐       │                            ┌─┐       │                                  ┌─┐       │
//!  │   │                └─┘───────┘                            └─┘───────┘                                  └─┘───────┘
//!  │ o │
//!  │ n │
//!  │   │
//!  │ s │      main has WTC before the switch         stash was raised and tracked                  - stash was raised and tracked
//!  │ w │                                             with WTC ref for main                           with WTC ref for feat
//!  │ i │                                                                                           - apply stash on main
//!  │ t │
//!  │ c │
//!  │ h │
//!  │   │
//!  └───┘
//!
//!
//!            ██████████████████ Apply feat and change WT ███████████████████████████ Unapply feat but WTC2 needs it █████████████████ switch to feat ██████████████████████████████████████
//!  ┌───┐
//!  │   │                                                    ┌──┐
//!  │   │                                                    │WS│◀─── H:WTC2:ws/1
//!  │ E │                                                    └──┘                                     ┌─┐                          ┌─┐
//!  │ p │   H:S:WTC:main                                       │                       H:S:WTC:main   └─┘◀──WTC/feat   WTC/main ──▶└─┘
//!  │ h │         │                                      ┌─────┴────┐                        │         │                            │
//!  │ e │         ▼                                      │          │                        ▼         │                            │
//!  │ m │        ┌─┐       ┌─┐                          ┌─┐        ┌─┐                      ┌─┐       ┌─┐                          ┌─┐       ┌─┐
//!  │ e │        └─┘       └─┘◀── feat         S:main ─▶└─┘        └─┘◀── S:feat            └─┘       └─┘◀── feat         main ───▶└─┘       └─┘◀──H:S:WTC
//!  │ r │         │         │                            │          │                        │         │                            │         │
//!  │ a │         │         │                            │          │                        │         │                            │         │
//!  │ l │         │         │                            │          │                        │         │                            │         │
//!  │   │        ┌─┐        │                           ┌─┐         │                       ┌─┐        │                           ┌─┐        │
//!  │ S │        └─┘────────┘                           WMB─────────┘                       └─┘────────┘                           └─┘────────┘
//!  │ t │
//!  │ a │                                                                               Some worktree changes only fit       stashed WTC were auto-applied
//!  │ s │                                         stash was raised from main,           onto feat, so they have been         upon switch.
//!  │ h │                                         then dropped again after              stashed onto it.
//!  │   │                                         switching to WS commit.
//!  │   │                                                                               It will be applied once the user
//!  │   │                                         Now there are new changes, WTC2       switches back
//!  └───┘
//!
//!
//!
//!
//!  ┌───┐     ██████████████████ Unapply the last Stack (main) ████████████████████████ Apply main (or switch to it) ████████
//!  │   │
//!  │   │         H:S:main                           main                                        H:S:main
//!  │ D │             │                                │                                             │
//!  │ e │             ▼                                ▼                                             ▼
//!  │ t │            ┌─┐                              ┌─┐                                           ┌─┐
//!  │ a │            └─┘                      HEAD ──▶└─┘                                           └─┘
//!  │ c │             │                                │                                             │
//!  │ h │             │                                │                                             │
//!  │   │             │                                │                                             │
//!  │ H │            ┌─┐                              ┌─┐                                           ┌─┐
//!  │ E │            └─┘                              └─┘                                           └─┘
//!  │ A │
//!  │ D │                              Unapplying the last stack from a
//!  │   │                              workspace detaches the HEAD.
//!  │   │
//!  └───┘                              No stack is available now.
//!
//!
//!
//!  ┌───┐     ██████████████████ Unapply the last Stack (main) ████████████████████████ Apply main (or switch to it) ████████
//!  │   │
//!  │   │         H:S:main                           main                                        H:S:main
//!  │ D │             │                                │                                             │
//!  │ e │             ▼                                ▼                                             ▼
//!  │ t │            ┌─┐                              ┌─┐                                           ┌─┐
//!  │ a │            └─┘                      HEAD ──▶└─┘                                           └─┘
//!  │ c │             │                                │                                             │
//!  │ h │             │                                │                                             │
//!  │   │             │                                │                                             │
//!  │ H │            ┌─┐                              ┌─┐                                           ┌─┐
//!  │ E │            └─┘                              └─┘                                           └─┘
//!  │ A │
//!  │ D │                              Unapplying the last stack from a             Now the HEAD isn't detached anymore.
//!  │   │                              workspace detaches the HEAD.
//!  │   │
//!  └───┘                              No stack is available now.
//!
//!
//!  ┌───┐     ██████████████████ List Stack Commits in WS ██████████
//!  │   │                  ┌──┐
//!  │ L │        H:ws/1 ──▶│WS│
//!  │ a │                  └──┘
//!  │ n │                    │                      ┌───┐┌───┐┌───┐
//!  │ e │             ┌──────┼──────┐               │ A ││ B ││ C │
//!  │   │             │      │      │               │┌─┐││┌─┐││┌─┐│
//!  │ C │            ┌─┐    ┌─┐    ┌─┐              │└2┘││└4┘││└5┘│
//!  │ o │   S:A   ──▶└2┘  ┌▶└4┘    └5┘◀─┐           │   ││┌─┐││   │
//!  │ m │             │   │  │      │   │           │   ││└3┘││   │
//!  │ m │             │   │  └─┬────┘   │           └───┘└───┘└───┘
//!  │ i │             │ S:B    │      S:C
//!  │ t │             │       ┌─┐          Each commit is only listed once, and
//!  │ s │             │       └3┘          consistently based on an algorithm.
//!  │   │             │        │
//!  │   │            ┌─┐       │           This also means that one has to handle
//!  └───┘            WMB───────┘           all commits at once.
//!
//!
//!
//!  ┌───┐     ██████████████████ Lis Commits in ordinary Merge ██████████████
//!  │   │                   ┌─┐
//!  │ L │       H:S:main──▶ └3┘
//!  │ a │                    │
//!  │ n │             ┌──────┼──────┐               main┐
//!  │ e │             │      │      │               │┌─┐│
//!  │   │             │      │      │               │└3┘│
//!  │ C │            ┌─┐    ┌─┐    ┌─┐              │┌─┐│
//!  │ o │            └2┘    └4┘    └5┘              │└2┘│
//!  │ m │             │      │      │               │┌─┐│
//!  │ m │             │      └─┬────┘               │└1┘│
//!  │ i │             │        │                    └───┘
//!  │ t │             │       ┌─┐
//!  │ s │             │       └6┘       First parent only traversal of merge
//!  │   │             │        │        commits.
//!  │   │            ┌─┐       │
//!  └───┘            └1┘───────┘        Problem is that lanes wouldn't have
//!                                      names otherwise.
//!
//!                                      Maybe one day we figure out something
//!                                      else, but this is safe.
//!
//!                                      One could imagine allowing to 'switch
//!                                      junctions', decide which parent to
//!                                      walk along.
//! ```

use crate::StashStatus;
use anyhow::{Context, bail};
use bstr::BString;
use but_core::RefMetadata;
use gix::prelude::ObjectIdExt;
use std::ops::{Deref, DerefMut};

/// The result of [`add_branch_to_workspace`].
#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    /// The new location of the workspace tip if the existing one was moved, or if a new one was created.
    /// This is equivalent to where `HEAD` should be.
    pub workspace_tip: gix::ObjectId,
    /// If we rebased something, this is the result of that operation.
    /// If `None`, this means `branch_id` was checked out.
    pub rebase_output: Option<but_rebase::RebaseOutput>,
}

/// Apply the given `branch_tip` so that it is part of the given `workspace_tip`,
/// which may also be initialized with `HEAD` and for all intends and purposes is equivalent to what `HEAD` points to.
/// After the operation, `branch_tip` should be part of the returned `workspace_tip`
/// If not available, a new tip will be provided which may be the commit the `branch` is currently pointing to,
/// indicating the workspace tip is now just the tip of an ordinary branch.
/// `target_tip`, if present, is the tip of the branch to integrate all workspace branches with ultimatley.
///
/// Note that at this point, the worktree will not have been touched, nor will references have been updated.
/// However, the `vb` will be updated where appropriate to match new ref positions in case the `branch` had to be
/// rebased.
pub fn add_branch_to_workspace(
    repo: &gix::Repository,
    branch_tip: gix::ObjectId,
    workspace_tip: gix::ObjectId,
    target_tip: Option<gix::ObjectId>,
    metadata: &mut impl RefMetadata,
) -> anyhow::Result<ApplyOutcome> {
    if crate::WorkspaceCommit::from_id(branch_tip.attach(repo))?.is_managed() {
        bail!("Cannot bring a workspace into another one")
    }

    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let merge_base = repo
        .merge_base_with_graph(workspace_tip, branch_tip, &mut graph)
        .context("Branch and workspace must have a merge-base")?;
    if merge_base == branch_tip || merge_base == workspace_tip {
        bail!("Cannot add branch that is already integrated")
    }
    todo!()
}

/// Like [`add_branch_to_workspace`], but will also update the involved references, and change `HEAD` to point to possibly newly
/// created `workspace_tip`.
/// `workspace_tip` can also be a detached `HEAD`, that's valid.
// TODO: maybe rather work with refs and add dry-run?
pub fn add_branch_to_workspace_and_update_refs(
    repo: &gix::Repository,
    branch_tip: gix::refs::PartialName,
    workspace_tip: gix::refs::FullName,
    target_tip: Option<gix::refs::PartialName>,
    metadata: &mut impl RefMetadata,
) -> anyhow::Result<ApplyOutcome> {
    todo!()
}

/// The inverse of [`add_branch_to_workspace`] where `workspace_tip` is the current `HEAD` and `branch_tip` is the tip to *not* include in the
/// `workspace_tip` anymore.
pub fn remove_branch_from_workspace(
    repo: &gix::Repository,
    branch_tip: gix::ObjectId,
    workspace_tip: gix::ObjectId,
    target_tip: Option<gix::ObjectId>,
    metadata: &mut impl RefMetadata,
) -> anyhow::Result<ApplyOutcome> {
    todo!()
}

/// An even more minimal version of the [`StackEntry](crate::StackEntry) with enough information to query
/// more information about a Stack.
///
/// Mote that a stack is also used to represent detached heads, which is far-fetched but necessary
// TODO: move this to the crate root once the 'old' implementation isn't used anymore.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Stack {
    /// The index into the parents-array of its [`WorkspaceCommit`](crate::WorkspaceCommit), but for our
    /// purposes just a way to refer to the stack.
    ///
    /// The actual index is dependent on the order in which they are merged into the workspace commit,
    /// if the stack is merged at all.
    // TODO: find a way to map this to (or provide) legacy StackIDs
    pub index: usize,
    /// The commit that the tip of the stack is pointing to.
    /// It is `None` if there is no commit as this repository is newly initialized, or no `base` is available.
    pub tip: Option<gix::ObjectId>,
    /// If there is an integration branch, we know a base commit shared with the integration branch from
    /// which we branched off.
    /// Otherwise, it's the merge-base of all stacks in the current workspace.
    /// It is `None` if this is a stack derived from a branch without relation to any other branch.
    pub base: Option<gix::ObjectId>,
    /// The branch-name denoted segments of the stack from its tip to the point of reference, typically a merge-base.
    /// This array is never empty.
    pub segments: Vec<StackSegment>,
    /// Additional information about possibly still available stashes, sitting on top of this stack.
    ///
    /// This means the stash is still there to be applied, something that can happen if the user switches branches
    /// using Git commands.
    ///
    /// The backend auto-applies floating stashes, but if that didn't happen, the frontend may guide the user.
    // TODO: refactor/remove this in favor of special stash commits.
    pub stash_status: Option<StashStatus>,
}

impl Stack {
    /// Return the name of the top-most [`StackSegment`].
    ///
    /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
    /// a commit anymore that was reached by our rev-walk.
    /// This can happen if the ref is deleted, or if it was advanced by other means.
    pub fn name(&self) -> Option<&gix::refs::FullNameRef> {
        self.segments
            .first()
            .and_then(|name| name.ref_name.as_ref().map(|name| name.as_ref()))
    }
}

/// A commit with must useful information extracted from the Git commit itself.
///
/// Note that additional information can be computed and placed in the [`LocalCommit`] and [`RemoteCommit`]
#[derive(Clone, Eq, PartialEq)]
pub struct Commit {
    /// The hash of the commit.
    pub id: gix::ObjectId,
    /// The IDs of the parent commits, but may be empty if this is the first commit.
    pub parent_ids: Vec<gix::ObjectId>,
    /// The complete message, verbatim.
    pub message: BString,
    /// The signature at which the commit was authored.
    pub author: gix::actor::Signature,
}

impl Commit {
    /// Read the object of the `commit_id` and extract relevant values.
    pub fn new_from_id(commit_id: gix::Id<'_>) -> anyhow::Result<Self> {
        let commit = commit_id.object()?.into_commit();
        // Decode efficiently, no need to own this.
        let commit = commit.decode()?;
        Ok(Commit {
            id: commit_id.detach(),
            parent_ids: commit.parents().collect(),
            message: commit.message.to_owned(),
            author: commit.author.to_owned()?,
        })
    }
}

impl std::fmt::Debug for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Commit({hash}, {msg:?})",
            hash = self.id.to_hex_with_len(7),
            msg = self.message
        )
    }
}

impl From<but_core::Commit<'_>> for Commit {
    fn from(value: but_core::Commit<'_>) -> Self {
        Commit {
            id: value.id.into(),
            parent_ids: value.parents.iter().cloned().collect(),
            message: value.inner.message,
            author: value.inner.author,
        }
    }
}

/// A commit that is reachable through the *local tracking branch*, with additional, computed information.
#[derive(Clone, Eq, PartialEq)]
pub struct LocalCommit {
    /// The simple commit.
    pub inner: Commit,
    /// Provide additional information on how this commit relates to other points of reference, like its remote branch,
    /// or the target branch to integrate with.
    pub relation: LocalCommitRelation,
    /// Whether the commit is in a conflicted state, a GitButler concept.
    /// GitButler will perform rebasing/reordering etc. without interruptions and flag commits as conflicted if needed.
    /// Conflicts are resolved via the Edit Mode mechanism.
    ///
    /// Note that even though GitButler won't push branches with conflicts, the user can still push such branches at will.
    pub has_conflicts: bool,
}

impl std::fmt::Debug for LocalCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LocalCommit({conflict}{hash}, {msg:?}, {relation})",
            conflict = if self.has_conflicts { "💥" } else { "" },
            hash = self.id.to_hex_with_len(7),
            msg = self.message,
            relation = self.relation.display(self.id)
        )
    }
}

impl LocalCommit {
    /// Create a new branch-commit, along with default values for the non-commit fields.
    pub fn new_from_id(value: gix::Id<'_>) -> anyhow::Result<Self> {
        Ok(LocalCommit {
            inner: Commit::new_from_id(value)?,
            relation: LocalCommitRelation::LocalOnly,
            has_conflicts: false,
        })
    }
}

/// The state of the [local commit](LocalCommit) in relation to its remote tracking branch or its integration branch.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum LocalCommitRelation {
    /// The commit is only local
    LocalOnly,
    /// The commit is also present in the remote tracking branch.
    ///
    /// This is the case if:
    ///  - The commit has been pushed to the remote
    ///  - The commit has been copied from a remote commit (when applying a remote branch)
    ///
    /// This variant carries the remote commit id.
    /// The `remote_commit_id` may be the same as the `id` or it may be different if the local commit has been rebased
    /// or updated in another way.
    LocalAndRemote(gix::ObjectId),
    /// The commit is considered integrated.
    /// This should happen when the commit or the contents of this commit is already part of the base.
    Integrated,
}

impl LocalCommitRelation {
    fn display(&self, id: gix::ObjectId) -> &'static str {
        match self {
            LocalCommitRelation::LocalOnly => "local",
            LocalCommitRelation::LocalAndRemote(remote_id) => {
                if *remote_id == id {
                    "local/remote(identity)"
                } else {
                    "local/remote(similarity)"
                }
            }
            LocalCommitRelation::Integrated => "integrated",
        }
    }
}

impl Deref for LocalCommit {
    type Target = Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for LocalCommit {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// A commit that is reachable only through the *remote tracking branch*, with additional, computed information.
///
/// TODO: Remote commits can also be integrated, without the local branch being all caught up. Currently we can't represent that.
#[derive(Clone, Eq, PartialEq)]
pub struct RemoteCommit {
    /// The simple commit.
    pub inner: Commit,
    /// Whether the commit is in a conflicted state, a GitButler concept.
    /// GitButler will perform rebasing/reordering etc. without interruptions and flag commits as conflicted if needed.
    /// Conflicts are resolved via the Edit Mode mechanism.
    ///
    /// Note that even though GitButler won't push branches with conflicts, the user can still push such branches at will.
    /// For remote commits, this only happens if someone manually pushed them.
    pub has_conflicts: bool,
}

impl std::fmt::Debug for RemoteCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "RemoteCommit({conflict}{hash}, {msg:?}",
            conflict = if self.has_conflicts { "💥" } else { "" },
            hash = self.id.to_hex_with_len(7),
            msg = self.message,
        )
    }
}

impl Deref for RemoteCommit {
    type Target = Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for RemoteCommit {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

/// A more detailed specification of a reference associated with a workspace, and it's location in comparison to a named reference point.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum RefLocation {
    /// The workspace commit can reach the given reference using a graph-walk.
    ///
    /// This is the common case.
    ReachableFromWorkspaceCommit,
    /// The given reference can reach into this workspace segment, but isn't inside it.
    ///
    /// This happens if someone checked out the reference directly and committed into it.
    OutsideOfWorkspace,
}

/// A list of all commits in a stack segment of a [`Stack`].
#[derive(Default, Clone, Eq, PartialEq)]
pub struct StackSegment {
    /// The name of the branch at the tip of it, and the starting point of the walk.
    ///
    /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
    /// a commit anymore that was reached by our rev-walk.
    /// This can happen if the ref is deleted, or if it was advanced by other means.
    pub ref_name: Option<gix::refs::FullName>,
    /// The name of the remote tracking branch of this segment, if present, i.e. `refs/remotes/origin/main`.
    /// Its presence means that a remote is configured and that the stack content
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
    /// Specify where the `ref_name` is specifically in relation to a workspace, or `None` if there is no ref-name.
    pub ref_location: Option<RefLocation>,
    /// The portion of commits that can be reached from the tip of the *branch* downwards, so that they are unique
    /// for that stack segment and not included in any other stack or stack segment.
    ///
    /// The list could be empty.
    pub commits_unique_from_tip: Vec<LocalCommit>,
    /// Commits that are reachable from the remote-tracking branch associated with this branch,
    /// but are not reachable from this branch or duplicated by a commit in it.
    ///
    /// Note that remote commits along with their remote tracking branch should always retain a shared history
    /// with the local tracking branch. If these diverge, we can represent this in data, but currently there is
    /// no derived value to make this visible explicitly.
    // TODO: review this - should branch divergence be a thing? Rare, but not impossible.
    pub commits_unique_in_remote_tracking_branch: Vec<RemoteCommit>,
    /// Metadata with additional information, or `None` if nothing was present.
    ///
    /// Primary use for this is the consumer, as edits are forced to be made on 'connected' data, so refetching is necessary.
    pub metadata: Option<but_core::ref_metadata::Branch>,
}

impl StackSegment {
    /// Return the top-most commit id of the segment.
    pub fn tip(&self) -> Option<gix::ObjectId> {
        self.commits_unique_from_tip.first().map(|commit| commit.id)
    }
}

impl std::fmt::Debug for StackSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let StackSegment {
            ref_name,
            ref_location,
            commits_unique_from_tip,
            commits_unique_in_remote_tracking_branch,
            remote_tracking_ref_name,
            metadata,
        } = self;
        f.debug_struct("StackSegment")
            .field(
                "ref_name",
                &match ref_name.as_ref() {
                    None => "None".to_string(),
                    Some(name) => name.to_string(),
                },
            )
            .field(
                "remote_tracking_ref_name",
                &match remote_tracking_ref_name.as_ref() {
                    None => "None".to_string(),
                    Some(name) => name.to_string(),
                },
            )
            .field(
                "ref_location",
                &match ref_location {
                    None => "None".to_string(),
                    Some(location) => {
                        format!("{:?}", location)
                    }
                },
            )
            .field("commits_unique_from_tip", &commits_unique_from_tip)
            .field(
                "commits_unique_in_remote_tracking_branch",
                &commits_unique_in_remote_tracking_branch,
            )
            .field("metadata", &metadata)
            .finish()
    }
}

/// Return all stack segments within the given `stack`.
pub fn stack_segments(stack: Stack) -> anyhow::Result<Vec<StackSegment>> {
    todo!()
}
