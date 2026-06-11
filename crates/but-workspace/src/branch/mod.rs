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
//! Note that any ref, like `refs/heads/gitbutler/workspace` can carry one or more stashes, so *workspace references* aren't special.
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
//!  ┌───┐     ██████████████████ List Commits in ordinary Merge ██████████████
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
use but_core::ref_metadata::{StackId, StackKind, WorkspaceCommitRelation::Outside};

use crate::ref_info;

/// A stack in the workspace, composed of one or more [segments](ref_info::Segment).
// TODO: move this to the crate root once the 'old' implementation isn't used anymore.
// TODO: this is going to be the UI version, ideally consumed directly.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Stack {
    /// If the stack belongs to a managed workspace, the `id` will be set and persist.
    /// Otherwise, it is `None`.
    pub id: Option<StackId>,
    /// If there is an integration branch, we know a base commit shared with the integration branch from
    /// which we branched off.
    /// Otherwise, it's the merge-base of all stacks in the current workspace.
    /// It is `None` if this is a stack derived from a branch without relation to any other branch.
    pub base: Option<gix::ObjectId>,
    /// The branch-name denoted segments of the stack from its tip to the point of reference, typically a merge-base.
    /// This array is never empty.
    pub segments: Vec<ref_info::Segment>,
}

impl Stack {
    /// Return the tip of the stack, which is either the first commit of the first segment or `None` if this is an unborn branch.
    pub fn tip(&self) -> Option<gix::ObjectId> {
        self.segments.first().and_then(|name| name.tip())
    }
    /// Return the name of the top-most [`Segment`](ref_info::Segment).
    ///
    /// It is `None` if this branch is the top-most stack segment and the `ref_name` wasn't pointing to
    /// a commit anymore that was reached by our rev-walk.
    /// This can happen if the ref is deleted, or if it was advanced by other means.
    pub fn name(&self) -> Option<&gix::refs::FullNameRef> {
        self.ref_name().map(|rn| rn.as_ref())
    }

    /// The same as [`name()`](Self::ref_name()), but returns the owned name.
    pub fn ref_name(&self) -> Option<&gix::refs::FullName> {
        self.segments
            .first()
            .and_then(|name| name.ref_info.as_ref().map(|ri| &ri.ref_name))
    }
}

/// What to do if the applied branch conflicts with the existing branches?
#[derive(Default, Debug, Copy, Clone)]
pub enum OnWorkspaceMergeConflict {
    /// Provide additional information about the stack(s) that conflicted and the files involved in it,
    /// and don't materialise the merge, but continue on best-effort basis to merge as many stacks as possible.
    #[default]
    AbortAndReportConflictingStacks,
    /// Despite possible conflicts, materialise the result with the conflicting stacks un-merged and unreachable from the workspace commit.
    /// Note that the metadata of these branches is still available.
    MaterializeAndReportConflictingStacks,
}

impl OnWorkspaceMergeConflict {
    /// Return `true` if we are supposed to abort on merge conflict.
    pub fn should_abort(&self) -> bool {
        matches!(
            self,
            OnWorkspaceMergeConflict::AbortAndReportConflictingStacks
        )
    }
}

/// Convert unnamed projected stacks into merge tips while preserving their parent order.
///
/// Named stacks are driven by workspace metadata, but anonymous stacks have no metadata entry
/// and must be supplied explicitly when rebuilding a workspace merge commit.
///
/// Each returned tuple is `(parent_index, tip)`: `parent_index` is the stack's position in the
/// projected workspace so the merge builder can insert the anonymous tip at the same parent slot,
/// and `tip` is the commit/segment pair to merge, with no ref name attached.
pub(crate) fn anon_stacks(
    stacks: &[but_graph::workspace::Stack],
) -> impl Iterator<Item = (usize, crate::commit::merge::Tip)> {
    stacks.iter().enumerate().filter_map(|(idx, s)| {
        if s.ref_name().is_none() {
            s.tip_skip_empty().and_then(|cid| {
                s.segments.first().map(|s| {
                    (
                        idx,
                        crate::commit::merge::Tip {
                            name: None,
                            commit_id: cid,
                            segment_idx: s.id,
                        },
                    )
                })
            })
        } else {
            None
        }
    })
}

/// Ensure every metadata stack that should be merged was visible in the graph.
pub(crate) fn ensure_no_missing_stacks(
    merge: &crate::commit::merge::Outcome,
) -> anyhow::Result<()> {
    if merge.missing_stacks.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Somehow some of the workspace stacks weren't part of the graph: {:#?}",
            merge.missing_stacks
        ))
    }
}

/// Map conflicting merge tips back to stable workspace stack ids.
///
/// Each entry in `conflicts` may carry the ref name of the tip that could not be merged. That ref
/// name is looked up in `ws_md` across both applied and unapplied stacks, and the owning stack's
/// stable id is returned.
///
/// Conflicts without a ref name, or whose ref name is no longer known to the
/// workspace metadata, are skipped.
pub(crate) fn correlate_conflicting_stack_ids(
    ws_md: &but_core::ref_metadata::Workspace,
    conflicts: &[crate::commit::merge::ConflictingStack],
) -> Vec<StackId> {
    conflicts
        .iter()
        .filter_map(|cs| cs.ref_name.as_ref())
        .filter_map(|ref_name| {
            ws_md
                .find_stack_with_branch(ref_name.as_ref(), StackKind::AppliedAndUnapplied)
                .map(|stack| stack.id)
        })
        .collect()
}

/// Mark conflicting stacks as outside the workspace and return their stable stack ids.
///
/// This is used when the caller chooses to materialize the best-effort merge result: the
/// conflicting branches remain known in metadata, but are no longer represented in the
/// checked-out workspace tree.
///
/// For each conflicting stack id given in `conflicts` and found in `ws_md`, this changes only
/// `WorkspaceStack::workspacecommit_relation` to `Outside`. The stack entry and its branch list
/// stay in `ws_md`, so the branch can still be re-applied later with its metadata intact.
pub(crate) fn correlate_conflicting_stack_ids_and_remove_from_workspace(
    ws_md: &mut but_core::ref_metadata::Workspace,
    conflicts: &[crate::commit::merge::ConflictingStack],
) -> Vec<StackId> {
    let conflicting_stack_ids = correlate_conflicting_stack_ids(ws_md, conflicts);
    for conflicting_id in &conflicting_stack_ids {
        let stack = ws_md
            .stacks
            .iter_mut()
            .find(|s| s.id == *conflicting_id)
            .expect("if it was found before it will be found as id");
        // TODO: this might as well be 'Unmerged' to keep them in the workspace, but not let them be merged.
        stack.workspacecommit_relation = Outside;
    }
    conflicting_stack_ids
}

/// Find `branch` in `repo` and reject it if it resolves to a symbolic reference.
///
/// `operation` is used only for the error message so callers such as apply and unapply can share
/// validation while still reporting the action they refused to perform.
///
/// Missing references are returned as `Ok(None)` so each caller can decide whether absence is an error or a no-op.
pub(crate) fn try_find_validated_ref<'repo>(
    repo: &'repo gix::Repository,
    branch: &gix::refs::FullNameRef,
    operation: &str,
) -> anyhow::Result<Option<gix::Reference<'repo>>> {
    let branch_ref = repo.try_find_reference(branch)?;
    if branch_ref
        .as_ref()
        .is_some_and(|r| matches!(r.target(), gix::refs::TargetRef::Symbolic(_)))
    {
        anyhow::bail!(
            "Refusing to {operation} symbolic ref '{}' due to potential ambiguity",
            branch.shorten()
        );
    }
    Ok(branch_ref)
}

/// Functions and types related to adding a branch to the workspace.
pub mod apply;
pub use apply::apply;

/// Tearing down the managed workspace so it can be rebuilt from scratch.
pub mod discard;
pub use discard::discard_managed_workspace;

/// Functions and types related to removing a branch from the workspace.
pub mod unapply;
pub use unapply::function::unapply;

/// related types for removing a workspace reference.
pub mod remove_reference;
pub use remove_reference::remove_reference;

/// related types for creating a workspace reference.
pub mod create_reference;
pub use create_reference::function::create_reference;

/// Functions and types related to moving branches across stacks.
pub mod move_branch;
pub use move_branch::function::{move_branch, tear_off_branch};

/// Functions and types for integrating remote changes into local branches.
pub mod integrate_branch_upstream;
pub use integrate_branch_upstream::{
    BranchIntegrationStrategy, InitialBranchIntegration, IntegrationDivergenceCommit,
    IntegrationDivergenceDisplay, IntegrationDivergenceTargetRelation, InteractiveIntegrationStep,
    get_initial_integration_steps_for_branch, integrate_branch_with_steps,
    parse_integration_steps_script, render_integration_steps_script,
};
