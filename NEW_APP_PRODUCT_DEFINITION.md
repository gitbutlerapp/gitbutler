# How Product Definition

Status: consolidated living draft

## One-Line Definition

How is not a simplified Git client. It is a way to manage changes while
building software.

## Manifesto

Git is infrastructure, not the product surface.

How is for people who are building software but should not need Git concepts
to keep working, return to earlier moments, or publish when they are ready. The
user should think in terms of Changes, Checkpoints, Bookmarks, publishing, and
the shared project. Branches, commits, stacks, rebases, force-pushes, remotes,
and conflict machinery are implementation details.

The product should feel calm, constrained, and obvious. It should not evoke risk
as the main reason to exist. It should simply make change manageable: nothing
gets lost, previous moments remain available, and publishing is optional.

How should earn trust by being small. Every visible action should have a
clear purpose, a predictable result, and a clear relationship to the user's
current Change.

## Audience

Primary audience: non-technical builders.

These are people building software who should not need Git concepts to feel
productive or in control. This includes vibe coders and less technical builders.
Regular developers who want less Git are a secondary audience, but they should
not pull the initial product toward a power-user Git surface.

## Relationship To Lite

How sits alongside `apps/lite`, but it is the counterpart to Lite rather
than a smaller copy of it.

Lite is the feature-rich GitButler app for people who want broad control over
workspace state, branches, commits, stacks, diffs, pull requests, conflict
resolution, and history editing.

How intentionally does less. It uses GitButler capabilities internally, but
it does not expose them as the user's normal operating model.

## First MVP: Local Checkpoint Loop

The first MVP lives at `apps/how` and proves the local Checkpoint capture loop
only.

MVP capabilities:

- Open one active project.
- Start one active project from a selected folder.
- Resume the last active project on launch.
- Watch project files.
- After 10 seconds of no further file changes, create an automatic Checkpoint.
- Show a simple Checkpoint timeline and save status.
- Restore the project back to an earlier Checkpoint.

MVP exclusions:

- No Publish UI.
- No diff viewer.
- No file list.
- No manual Checkpoints.
- No secret scanning.

MVP Checkpoints are implemented as normal Git commits in the current branch
history. The UI still calls them Checkpoints, never commits.

MVP Checkpoint commit messages use the format:

```text
Checkpoint: <AI title or local timestamp>

<optional AI summary body>
```

MVP meaningful-change detection uses Git's own status after the 10-second quiet
period. If Git reports no changes, How does not create a Checkpoint.

Because the current SDK does not expose every desired product-level operation,
the first implementation composes lower-level capabilities in How's Electron
process. It uses GitButler SDK calls for worktree inspection, watcher events,
and other application integration points where useful. Checkpoint creation uses
narrow Git CLI calls in Electron for now, including repository discovery,
repository initialization, timeline reads, commits, and restoring to a previous
Checkpoint.

## Core Concepts

**Change**:
The user's current task, versioned over time. A Change can be something like
"fix login", "homepage edits", or "try the new dashboard layout". V1 supports
one active Change at a time. Internally, a Change may map to GitButler branches,
stacks, commits, snapshots, or hunk assignments, but those are not user-facing
objects.

**Checkpoint**:
An automatically created previous moment within the current Change. A Checkpoint
means "you can come back here." The app should not call Checkpoints commits.

**Bookmark**:
A named saved state the user can intentionally return to. A Bookmark is backed
by a private Git ref, but the product surface should not call it a branch, ref,
or version.

**Publish**:
The act of sending the current Change to the GitHub project destination. V1 has
one GitHub-backed publish flow.

**Project destination**:
The place the project is published to. Use this instead of "remote" in the
product surface.

**Shared project**:
The published project state on GitHub. Use this instead of `main` or
`origin/main` in the product surface.

## V1 Product Loop

1. User opens an existing project or starts a new project.
2. App initializes or validates versioning invisibly.
3. App shows the current Change screen.
4. User edits in their coding tool or AI coding agent.
5. App detects edits and automatically creates Checkpoints.
6. User can restore to a previous Checkpoint.
7. User can publish when ready, or not publish at all.

Publishing belongs in the first complete product loop. It should not feel like a
separate power feature.

## Main Screen

V1 is essentially one calm Change screen:

- Current project name.
- Current Change status, such as "Saved just now", "Saving...", "Unsaved
  changes", "Update available", or "Could not save".
- Time-based Checkpoint timeline for unpublished local work, with an
  AI-generated label when available.
- Primary action: Publish.
- Shared-project action: Update project, shown when the shared version has
  changed.
- Checkpoint action: Restore.
- Optional small notice when the shared version has changed.

V1 should not show file lists, diffs, branch lists, commit lists, graphs,
staging, or other source-control machinery.

## Checkpoints

Checkpoints are automatic only in v1. The user should not need to remember to
save Checkpoints or decide when versioning ceremony is appropriate.

The desired feeling is Google Docs autosave for code:

- If a code-generation hook reports completion, create a Checkpoint immediately
  after generation completes.
- Otherwise, when file-watcher changes arrive, wait for 10 seconds with no
  further file changes.
- After the quiet period, create a Checkpoint only if there is a meaningful
  diff.
- Coalesce repeated file changes while the user or agent keeps editing.
- Avoid Checkpoints for ignored files, build output, metadata churn, cache
  noise, or other non-user work.

Manual Checkpoints are not part of v1. They are not ruled out forever, but the
product definition does not yet know what a simple manual flow should look like.

Users cannot delete, rename, pin, or manually clean up Checkpoints in v1.
Retention and cleanup can be automatic/internal later.

The Checkpoint timeline should show only local work that has not yet reached the
shared project. Once How has refreshed its upstream knowledge and a Checkpoint is
reachable from the configured shared-project upstream, that Checkpoint is hidden
from the normal timeline. The user should not have to see or manage published
Checkpoints as local work.

## AI Checkpoint Summaries

When a project has a coding agent configured, How may ask that same agent to
summarize the changes being saved into a Checkpoint. This is best-effort
orientation, not a requirement for saving.

MVP behavior:

- Summaries are attempted only when `how.codingAgent` is `codex` or `claude`.
- `none` means no AI attempt.
- How should use the actual agent SDKs for the configured coding agent:
  `@openai/codex-sdk` for Codex and `@anthropic-ai/claude-agent-sdk` for
  Claude.
- The SDKs should use the user's existing local agent setup and authentication.
  How does not collect, store, or configure API keys.
- How does not expose model selection in the UI.
- The summary input is the saved Checkpoint commit patch. How creates the
  Checkpoint first, then summarizes the saved patch asynchronously.
- The diff payload is capped at roughly 40 KB. If truncated, the prompt tells
  the agent to summarize only the visible diff.
- Summary generation is read-only. The agent should not edit files or inspect
  the project beyond the diff How provides.
- How never waits for AI before saving. The initial Checkpoint is created
  immediately with a timestamp title.
- If summary generation succeeds later, How updates the Checkpoint commit
  message by its explicit GitButler Change ID and refreshes the timeline.
- If summary generation fails, times out, is cancelled, or the Checkpoint is no
  longer safe to rewrite, the timestamp title remains.
- Summary failures are silent in the UI when the fallback Checkpoint succeeds.
  Logs should include the selected agent, diff size, truncation state, timeout,
  and fallback reason, but not the full diff or generated summary.
- Async message updates are local-only. How must not rewrite Checkpoints that
  are already reachable from the configured published/shared branch.
- Async generation may run in parallel, but Git message update operations are
  serialized. How resolves the current commit by Change ID at apply time.
- Pending async summary updates are cancelled when the active line changes:
  switching Bookmarks, entering or leaving browsing, opening/starting/deleting a
  project, or successfully publishing.

Agent output is strict plain text. The first line is the title. Remaining text
is the optional body. How owns the `Checkpoint:` prefix, strips an accidental
agent-provided `Checkpoint:` prefix from the title, and stores the body only in
the Git commit message body.

New Checkpoints must store an explicit GitButler `change-id` commit header.
That Change ID is How's durable Checkpoint identity for async message updates.
Old Checkpoints without explicit Change IDs remain visible, but How does not
attempt async message updates for them.

The MVP implementation keeps provider wiring isolated behind a small
Electron-only summarizer abstraction. Automated tests use an env-gated fake
provider so CI does not depend on local agent authentication, network access, or
real model latency.

## Checkpoint Timeline

The first version shows a simple time-based timeline. AI labels are optional and
best-effort.

Examples:

- "Saved just now"
- "Saved 2 minutes ago"
- "Updated checkout form" if an AI label is available
- "Published"
- "Review created"

AI labels must never delay Checkpoint creation and never prevent saving,
restoring, or publishing. The app should not require account or AI setup before
it is useful.

No diff viewer exists in v1. The timeline is for orientation and returning to
earlier moments, not for code review.

## Checkpoint Browsing And Restore

Restore is a full-project return to a selected Checkpoint, but the user should
not be forced to choose the new direction immediately.

Because v1 has no diff viewer, Restore must be forgiving. Going back is split
into two distinct ideas:

- **Browsing Checkpoints**: temporarily viewing an earlier Checkpoint without
  choosing a new direction.
- **Continue from here**: deciding that the currently viewed Checkpoint is the
  new place to build from.

Before entering browsing mode, How creates a Checkpoint for the current state if
there are unsaved changes. If that Checkpoint cannot be created, the app does not
enter browsing and shows a plain-language error.

Browsing never changes history. While browsing, How pauses automatic Checkpoint
creation so a local test or accidental edit does not silently turn inspection
into a new direction. The timeline should keep showing the full Checkpoint chain
the user was browsing from, so the user can move from A to B to C before choosing
where to continue.

If the user changes files while browsing, How should not autosave. Instead, it
marks the state as changed while browsing. If the user tries to view another
Checkpoint, return to the latest Checkpoint, switch projects, or delete the
project from How, How asks what to do in plain language:

- **Leave changes**: discard the browsing edits and move to the requested
  Checkpoint or latest state.
- **Cancel**: stay at the current browsed Checkpoint with the edits intact.

**Continue from here** is a separate explicit action in the browsing UI. If the
browsed state has edits, it creates a Checkpoint from the browsed Checkpoint plus
the edits, exits browsing, and makes that new Checkpoint the current state. If
the browsed state is clean, it exits browsing at the selected Checkpoint without
creating another Checkpoint.

Going back is reversible until the user explicitly starts building from the
earlier Checkpoint by choosing **Continue from here**. Once they continue, How
treats that as the new direction while preserving the old direction in
recoverable project history.

The MVP browsing implementation persists browsing state in How's local Electron
state so the app can restart while still knowing that autosave is paused and
which Checkpoint is being viewed. This is intentionally not the final history
model. A future design should preserve "went back from / can go forward to"
information in a way that remains readable from Git state as much as possible,
rather than hiding the recovery path inside app-only state.

## Bookmarks

Bookmarks are intentional saved states. They are different from Checkpoints:
Checkpoints are automatic timeline entries inside the current Change, while
Bookmarks are user-recognizable places the user can switch to later.

Each Bookmark represents the full Checkpoint history reachable from its saved
state, not just a single file snapshot. Two Bookmarks may share older
Checkpoints and then diverge as the user switches and keeps building from one of
them.

Opening or switching to a Bookmark changes the current project state to that
Bookmark and future Checkpoints continue from there. Before switching, How
preserves the current state as another Bookmark only when that state is not
already represented by an existing Bookmark. Repeatedly switching away from an
already bookmarked state should not create duplicate Bookmarks.

Switching to a Bookmark does not make that Bookmark follow future Checkpoints.
The Bookmark remains pointed at the saved commit until the user explicitly
updates it. After new Checkpoints are created from that starting point, the
Bookmark no longer represents the current state and should no longer be marked
current.

Creating new Checkpoints after switching to a Bookmark does not automatically
create another backup Bookmark. The original Bookmark already preserves the
starting state. The new active state remains unbookmarked until the user creates
a Bookmark or switches away and the switch-preserve rule applies.

Switching to a Bookmark should feel as fast as possible without losing work.
For a clean active state, clicking a Bookmark switches immediately. If the
worktree is dirty, How first creates a timestamp Checkpoint without scheduling
AI message generation, then preserves the resulting active state as a backup
Bookmark if no existing Bookmark already points to it, and then switches. If
saving fails, How does not switch.

Bookmark switching fails closed. If checkout/reset, unusual repository state, or
conflicts prevent switching, How keeps the user at the current state and shows a
plain-language error. If a backup Bookmark was already created before the
failure, it remains visible because it accurately preserved the previous state.

Auto-preserved Bookmarks are visible in the normal Bookmark list. They should
use a clear system-generated name, such as "Before switching to <Bookmark name>"
or a timestamp-based fallback, so the user can trust that the previous state is
recoverable. Rename, delete, or archive controls can come later if the list gets
too noisy.

The Bookmark list shows all Bookmarks, including any Bookmark that points to the
current active state. That Bookmark should be marked as current, and clicking it
is a no-op.

Bookmarks live in a project sidebar rather than as the primary center content.
The Checkpoint timeline remains the main interaction center. The Bookmark list
may grow large, so it should not take over the main area or reduce the timeline's
importance. The sidebar includes the **Bookmark current state** action, the list
of Bookmarks with the current marker, and per-Bookmark actions such as **Update
to current state**.

On desktop-sized project screens, the Bookmark sidebar lives on the left and
scrolls independently from the Checkpoint timeline. The Checkpoint timeline keeps
its own scroll area so a long Bookmark list never steals the main timeline's
working space.

When there are no Bookmarks, the sidebar has a compact empty state: "No
bookmarks" and the **Bookmark current state** action. It should not use long
educational copy or compete with the Checkpoint timeline.

The first Bookmarks implementation does not need search or filtering. Sorting,
rename, and delete are enough to make the first version usable; larger-list UX
can be refined after the core behavior works.

Users can delete Bookmarks in the first Bookmarks implementation. Deleting a
Bookmark is a confirmed destructive action that removes the Bookmark pointer; it
does not immediately delete underlying Checkpoint commits. This cleanup applies
to Bookmarks only. Checkpoints remain unmanaged by users in v1.

Deleting the current Bookmark is allowed with confirmation. It removes only the
Bookmark pointer and leaves the current project state unchanged. After deletion,
there may be no current Bookmark; automatic Checkpoints continue on the active
line.

Users can rename Bookmarks in the first Bookmarks implementation. Rename updates
Bookmark metadata only, not the private ref ID. It is non-destructive and does
not require confirmation.

Bookmarks are local-only in the first implementation. Publishing publishes the
active project state, not the set of Bookmarks. Internal storage and publish
paths should make it hard to push Bookmarks accidentally.

Publishing is Bookmark-neutral. It does not update the current Bookmark, create
a Bookmark, delete Bookmarks, or push Bookmark refs. Publishing only operates on
the active project state.

Under the hood, a Bookmark is backed by a private Git ref outside
`refs/heads`, such as `refs/gitbutler/how/bookmarks/<id>`. The UI should still
call it a Bookmark and should not expose ref mechanics.

For How's constrained model, the repository's `main` branch is the single
internal active working line. Bookmark refs hold saved states. Switching to a
Bookmark first ensures the current `main` tip is represented by a visible
Bookmark unless it already is, then moves `main` to the selected Bookmark tip,
updates the worktree, and continues automatic Checkpoints on `main`.

Bookmarks have stable internal IDs separate from their display names. Display
names should be friendly and editable later; backing refs should use generated
IDs so Git ref-name rules, duplicate names, and future renames do not leak into
the product surface.

Bookmark metadata is stored locally as JSON under `.git/gitbutler` in the first
implementation. This metadata includes display name, creation/update timestamps,
and whether the Bookmark was user-created or auto-preserved. Later, How should
migrate this metadata to git-meta so Bookmark information can be encoded and
shared when the product is ready for shared Bookmarks.

Bookmark display names are not unique identifiers. How may allow duplicate
names, but the creation UI should nudge toward clarity with suggestions such as
"Name 2" or an inline warning when a name already exists. Stable IDs remain the
source of identity.

Auto-preserved Bookmarks keep `kind: "auto"` until the user takes ownership of
them. Renaming an auto Bookmark or manually updating it promotes it to
`kind: "user"`, which also moves it into the intentional Bookmark group for
sorting. Merely switching to or from an auto Bookmark does not promote it.

The first Bookmarks implementation supports an explicit **Bookmark current
state** action. If the worktree has unsaved changes, How creates a Checkpoint
first and then points the new Bookmark at that resulting active state. If there
are no unsaved changes, the Bookmark points at the current active state without
creating another Checkpoint.

How does not create a default Bookmark automatically when a project is opened or
started. Bookmarks are intentional: the list may be empty until the user creates
one or until How auto-preserves a state during Bookmark switching.

Bookmark representation checks use commit identity only. A state is already
represented by a Bookmark when an existing Bookmark ref points to the same
commit ID. How should not use tree/content equivalence for this decision.

While browsing an old Checkpoint, users can create a new Bookmark from the clean
browsed Checkpoint without choosing **Continue from here**. This saves an
interesting old state without changing the current direction. Users cannot
update an existing Bookmark from browsing mode, and dirty browsing edits must be
continued from before they can be bookmarked or used to update a Bookmark.

Users can explicitly update any existing Bookmark to the current state without
switching to it first. Updating a Bookmark is a destructive replacement action
on that Bookmark, such as **Update to current state**, and requires a
confirmation dialog. If the worktree has unsaved changes, How creates a fast
Checkpoint first, then moves the Bookmark to the resulting active state. How
does not automatically preserve the Bookmark's previous target; users can create
another Bookmark first when they want to keep it.

## Project Settings

Project settings are per-project local preferences stored in the project's local
Git config, not in committed project files.

MVP settings:

- `how.checkpointDebounceMs`: automatic Checkpoint quiet period in milliseconds.
  The UI shows integer seconds from 1 to 60. The default is 10 seconds.
- `how.codingAgent`: preferred coding agent. Values are `none`, `codex`, or
  `claude`. The default is `none`.

The coding agent setting controls best-effort AI Checkpoint summaries. It does
not launch or configure an interactive coding agent.

Missing or invalid Git config values silently fall back in the UI. Saving
settings writes normalized values back to local Git config.

Settings live on a separate route and are opened from a small gear button next
to the project title. Settings are available while browsing Checkpoints,
including dirty browsing, because changing these preferences does not leave the
browsing state.

Changing the debounce setting applies immediately. If a Checkpoint save is
already running, it is allowed to finish. If a save is only pending, How
reschedules it using the new debounce value.

## Publish

V1 has one publish flow: GitHub-backed publishing to a project destination.
There is no review/direct mode choice in the first product surface.

Publishing always requires the user to connect or log in to GitHub from How. The
user should not paste a remote URL into the application. After GitHub is
available, the user chooses an existing GitHub repository or creates a new one.
How then configures the repository as the project destination and publishes the
current project state.

Use GitHub only in v1. GitHub Enterprise, fork-based review flows, providers
other than GitHub, and review publishing are future work.

Publish always creates a Checkpoint immediately before doing any update or
publish work when there are unsaved changes. This captures the exact state the
user intended to publish, even if the quiet-period autosave had not fired yet.

MVP GitHub publish does not yet create a durable timeline marker or amend the
pre-publish Checkpoint with the publish outcome. A successful publish updates
the status text to "Published just now" only. Durable publish markers remain a
future Git-readable design problem.

Publishing is disabled while browsing Checkpoints. The user must choose
**Continue from here** or **Return to latest** before publishing.

Use plain language:

- "Project destination", not "remote".
- "Choose a GitHub project", not "select a remote repository".
- "Create a GitHub project", not "create a remote".
- "Shared project", not `main` or `origin/main`.

## GitHub Integration

How's first forge integration is GitHub on `github.com` only.

How's first implementation should not use the existing
`listKnownGithubAccounts` account-discovery API. How should store credentials in
How-owned secure credential storage, separate from GitButler's existing GitHub
account storage. How should not use or mutate the existing GitButler account
list for the first implementation.

The How Electron process may use the GitHub TypeScript SDK for product
operations such as completing OAuth device login, listing repositories, creating a
repository, and publishing to the selected GitHub project. These APIs can move
to Rust later.

Logging in to GitHub uses OAuth device flow. How should not ask the user to
paste a Personal Access Token in the first product UI. After login succeeds, How
reads the authenticated `github.com` user and stores the credential in
How-owned app-wide secure storage.

The selected GitHub project destination is project-specific. A project may store
which How-owned credential it uses as a local reference:

```text
how.githubCredential = <credential identifier>
```

The credential itself must never be stored in Git config or committed project
files. The credential identifier should be opaque and local to How. The visible
account label should be the GitHub login, which How may store alongside the
app-wide credential metadata for display. How does not show an account picker in
the first implementation. If the stored GitHub credential is missing, expired,
or lacks permission, How asks the user to log in to GitHub again.

The app-wide GitHub credential should persist across How restarts.

If GitHub login succeeds but the credential lacks permission to list projects,
create projects, or publish, How shows plain language such as: "How could not
get permission to publish with this GitHub login." The main UI should not teach
OAuth scopes in v1. Logs may include GitHub's technical reason.

There is no visible GitHub logout or account-management UI in the publish slice.
If the user logs in again, How replaces the stored app-wide GitHub credential.
Future settings can expose credential management if needed.

If the user is not logged in when they click Publish, How shows a small dialog:

- Title: **Publish with GitHub**
- Body: "Log in to choose where this project publishes."
- Primary action: **Log in to GitHub**
- Secondary action: **Cancel**

The dialog should not mention OAuth, tokens, remotes, branches, or repository
URLs.

## GitHub Project Destination

When publishing needs setup, How should guide the user through GitHub rather than
asking for a URL.

There is no user-facing manual project destination URL fallback in v1. Electron
internals and tests may still receive clone URLs from the GitHub service, but
the product UI should not ask the user to paste one.

If the project already has a GitHub destination configured before it is opened in
How, How may use that destination. Publishing still requires the user to be
logged in to GitHub through How. If the logged-in account cannot publish to that
destination, How shows a plain-language permission error.

If the current branch already tracks one GitHub destination, How publishes
there. If there are multiple GitHub destinations but the current branch does not
track one, How should not guess. It should ask the user to choose an existing
GitHub project or create a new one, then configure the current branch to publish
there.

If the project already has a non-GitHub destination configured, How does not
overwrite it in v1. It stops with plain language such as: "This project already
publishes somewhere How does not support yet."

The setup choices are:

1. **Create GitHub project**:
   Primary path. Create a new private GitHub repository, add it as the project
   destination, and publish there.
2. **Choose existing GitHub project**:
   List repositories the authenticated GitHub user can publish to. In
   implementation terms, prefer repositories where GitHub reports push or admin
   permission, regardless of whether the repository is public or private. The UI
   should not expose those permission terms. The user selects one. How adds it
   as the project destination and publishes there. If GitHub's permissions are
   not conclusive, How may handle a later publish rejection with a plain-language
   error.
   Existing project selection should be one searchable list using `owner/name`
   labels. Do not split the UI into personal repositories, organizations, or
   other GitHub taxonomy in v1.
   Before using an existing GitHub project, How should do a simple compatibility
   preflight. Empty repositories are valid. Repositories with compatible history
   are valid. If the GitHub project already has files or history How cannot
   safely publish over, How stops with plain language such as: "That GitHub
   project already has files How cannot publish over."
   Load this list only after the user clicks **Choose existing project**.
   When no destination is configured after GitHub login, How shows:

- Title: **Where should this publish?**
- Primary action: **Create GitHub project**
- Secondary action: **Choose existing project**

Created GitHub projects are private by default in v1. There is no public/private
choice in the first pass. How creates an empty repository with no GitHub-created
README, license, `.gitignore`, or description because the local project already
has the files and history to publish.

Created GitHub projects live under the authenticated GitHub user's personal
account in v1. Organization creation is future work. Existing organization
repositories may still appear in **Choose existing GitHub project** when the
authenticated user can publish to them.

The GitHub project name is pre-filled from the local folder name and can be
edited before creation. How should always show this editable confirmation screen
before creating a GitHub project; it should not silently create a repository from
the folder name. How validates only simple GitHub-safe names locally. If GitHub
reports that the name is already used, How shows a plain inline error and lets
the user choose another name. How should not silently auto-suffix GitHub project
names.

How adds the selected or created GitHub project destination using an HTTPS clone
URL returned by GitHub, adds it as `origin` if no destination exists yet, pushes
the current branch, and sets upstream tracking. How does not store the project
destination URL in How-specific config; normal Git remote and upstream config
remain the source of truth.

How pushes the current supported branch as-is and sets tracking. It should not
rename branches as part of publishing, and it should not expose branch names in
the product UI.

How may cache a display-only GitHub project label later if useful, but Git
remote/upstream configuration remains authoritative for where the project
publishes.

The GitHub credential How obtains should authenticate both GitHub API calls and
the actual `git push`. How should not embed tokens in remote URLs, and it should
not ask the user for a separate Git credential prompt after they have logged in
to GitHub in How.

After a project destination has been selected or created once, future Publish
clicks should skip setup and publish there directly as long as the GitHub
credential and destination are still valid.

Publishing creates a Checkpoint before pushing only when there are unsaved
changes. If that Checkpoint cannot be created, publishing stops. If pushing
fails, the Checkpoint remains because it captured the intended publish state.

Publishing never force-pushes. It does not automatically pull, merge, rebase, or
move the local active line in the MVP. If the shared project has changed, Publish
is disabled and explains that the project should be updated first.

After a successful Publish, How should refresh its upstream knowledge for the
configured shared-project destination and refresh the timeline. This is not an
active-line update: the local active line should already match what was pushed.
The refresh lets How recognize that the just-published Checkpoints are now part
of the shared project and should disappear from the local-work timeline.

## Future Review Publishing

Review publishing is intentionally out of the current V1 implementation. The
previous design direction remains useful later: How may eventually send the
current Change to GitHub for review, then return the local project to the shared
trunk so the user can keep building on the normal project line. Stacked reviews,
review rework, draft reviews, and fork-based reviews are all future work.

## GitHub Publish Testing

End-to-end tests should click the real How UI for publishing, including GitHub
login, existing project selection, and project creation. The GitHub boundary
should be env-gated and fake in tests: fake OAuth success, fake authenticated
login, fake repository listing, and fake repository creation. Git operations
should still use real local repositories and bare repositories where possible so
the publish behavior is exercised without real GitHub network calls.

## Bookmark Testing

The first Bookmark implementation needs targeted Rust API tests and How
end-to-end tests.

Rust/API tests should cover:

- Creating and listing a Bookmark with metadata and a private ref.
- Switching to a Bookmark moves How's internal active line to the Bookmark tip.
- Updating a Bookmark moves its private ref to the active state.
- Deleting a Bookmark removes the pointer only.
- `isCurrent` is computed by commit ID.
- Duplicate display names are allowed.

How end-to-end tests should cover:

- Bookmarking the current state from clean and dirty states.
- Switching to a Bookmark preserves the previous state only when no existing
  Bookmark points to that commit ID.
- Dirty switching creates a fast Checkpoint without AI summary generation.
- The current Bookmark marker updates after new Checkpoints.
- Updating an existing Bookmark shows a confirmation dialog.
- Deleting the current Bookmark leaves files unchanged.
- Publishing does not push, create, update, or delete Bookmarks.

Run How end-to-end tests headlessly whenever validating the app. Use:

```sh
pnpm --filter @gitbutler/how exec cross-env HOW_E2E_HEADLESS=1 playwright test --config e2e/playwright.config.ts
```

Do not run the How e2e suite headed by default; headed runs are only for
intentional local debugging.

## Shared Project Freshness

How should keep its knowledge of the shared project fresh without moving the
user's files unexpectedly.

The MVP includes read-only shared-project fetching and timeline filtering:

- How fetches the configured shared-project upstream continuously.
- The default fetch interval is 15 minutes.
- The interval is a project setting named **Check for shared updates** with
  choices: Off, 5 min, 15 min, 30 min, and 60 min.
- The setting is stored in local repository config as milliseconds, for example
  `how.fetchIntervalMs`, and applies immediately.
- Passive fetch failures should not interrupt the user. How logs them and may
  show a soft "Could not check for updates" state only if useful.
- Publish and manual update actions show direct plain-language errors because
  those are user-initiated.
- Background fetch is read-only and low priority. It must not interrupt
  autosave, mutate the active line, or open login prompts.

After each successful fetch, How computes whether the active line is behind the
shared project. Use the current branch's configured upstream as the source of
truth; in the normal How path that is expected to be `origin/main`, but the UI
should still say "shared project". If no upstream is configured, How cannot check
for shared updates until publishing configures the project destination.

The active line needs an update when its merge base with the configured upstream
is not the upstream tip. In that state:

- The main UI shows **Update available**.
- The top action area shows **Update project**.
- Publish is disabled with a tooltip such as "Update this project before
  publishing."
- Checkpoint autosave can continue normally.

How does not eagerly determine whether every Bookmark is on an old base. When
the user switches to a Bookmark, How evaluates the newly active line against the
configured upstream. If that active line is based on an older shared-project
state, How shows the same **Update available** state. Switching Bookmarks remains
a local operation; updating is separate and explicit.

The near-future gap is the actual **Update project** operation. When implemented,
it should:

- Be disabled while browsing Checkpoints.
- Let any in-progress Checkpoint save finish before updating.
- Create a normal Checkpoint first when there are unsaved changes.
- Replay/rebase only the active line's local unpublished Checkpoints on top of
  the updated shared project.
- Leave Bookmarks untouched.
- Never rewrite anything reachable from the configured upstream.
- Never force-push.
- If the active line has no local work and is simply behind, fast-forward or
  reset it to the shared project.
- If conflicts or unsupported history shapes appear, leave the project at the
  original pre-update state and show a plain-language error.

Implementation should first evaluate whether the existing
`getInitialBranchIntegration` and `applyBranchIntegration` APIs fit this model
using the rebase-style strategy. How should not expose merge, pick-remote,
smart-squash, or other strategy choices. If those APIs require user choices,
conflict handling, or cannot provide rollback guarantees, How needs a dedicated
shared-project update API.

## Errors And Unsupported States

Errors should only use plain language. Do not expose Git terminology as primary
copy or secondary technical detail in v1.

Use language like:

- "The shared version changed in a way this app cannot update automatically."
- "This project has work in a shape this app does not support yet. Your files
  were not changed."

Avoid language like:

- "Rebase failed due to conflicts."
- "Merge conflict while updating from `origin/main`."

V1 has no advanced mode and no "Open in Lite" escape hatch. If the app
encounters complexity it cannot handle, it should error clearly and leave the
project unchanged.

Unsupported v1 states include multiple active branches/Changes, unresolved
conflicts, detached or otherwise unusual state, existing unpushed branch work,
or anything that does not fit the one-current-Change model.

## First Run And Eligible Projects

First run offers only:

1. Open an existing project.
2. Start a new project.

The app should not ask for publish mode, remote setup, account setup, Change
naming, or Git concepts during first run.

V1 should support brand-new non-Git folders as well as simple existing
repositories. Non-technical builders may not start from Git, so the app should
quietly initialize versioning when needed.

"Start new project" means create or select a folder and initialize invisible
versioning. The app should not scaffold applications, offer templates, or become
a project generator. The user's coding tool or AI agent creates the actual
project files.

Initial eligibility:

- Local project folder.
- Git repository already exists, or the app can initialize versioning.
- One trunk/shared line can be identified or created.
- No unsupported existing work state.
- Project destination is optional until Publish.
- GitHub integration is optional until Publish.
- GitButler workspace setup can happen invisibly if needed.

## V1 Non-Goals

- No multiple active Changes.
- No manual Checkpoints.
- No Change naming.
- No diff viewer.
- No file-level selection.
- No file list as a core surface.
- No branch, commit, stack, rebase, force-push, remote, `main`, or `origin/main`
  terminology in the normal product surface.
- No advanced mode.
- No Lite handoff in v1.
- No conflict resolution.
- No force-push.
- No history editing.
- No pull request state management beyond creating a review on GitHub.
- No reviewing code inside the app.
- No Checkpoint deletion, renaming, pinning, or manual cleanup.
- No project templates or scaffolding.
- No providers beyond GitHub in v1.

## Existing GitButler Capabilities To Use Internally

The current `but`/Lite platform can already support many operations that should
mostly remain invisible in this app:

- Inspect workspace state, diffs, branches, commits, and file/hunk-level
  changes.
- Create, apply, unapply, rename, delete, stack, and tear off branches.
- Assign hunks and commit selected files or hunks.
- Amend, absorb, squash, move, reword, uncommit, discard, and otherwise edit
  commit history.
- Push branches, create pull requests, set draft/ready states, and configure
  forge integration.
- Pull/update applied branches onto the target branch.
- Resolve conflicts through a dedicated resolution mode.
- Use an operation log with undo, redo, snapshots, and restore.
- Prompt for credentials through the Lite askpass bridge.

The product question is not whether these capabilities exist. It is which become
invisible automation, which become simple user actions, and which are
intentionally omitted.

## Missing Or Future `but-sdk` APIs

The first MVP can compose lower-level primitives in `apps/how/electron`, but How
should eventually consume product-level SDK APIs so the application boundary
also treats Git as an implementation detail.

Desired future APIs:

- `addProjectBestEffort(path)` exposed in the JavaScript SDK.
- `addProject(path)` exposed in the JavaScript SDK.
- `getProject(projectId)` exposed in the JavaScript SDK.
- `initRepository(path)` or `startProject(path)` for brand-new non-Git folders.
- `prepareProjectForActivation(projectId)`, or a combined open/start API that
  performs activation.
- `createCheckpoint(projectId, options)` as a stable product abstraction.
- `listCheckpoints(projectId, limit?)`.
- `restoreCheckpoint(projectId, checkpointId)`.
- `browseCheckpoint(projectId, checkpointId)`.
- `continueFromCheckpoint(projectId, options)`.
- `returnToLatestCheckpoint(projectId)`.
- `summarizeCheckpoint(projectId, options)`, or a stable agent summarization
  abstraction backed by configured coding agents.
- Bookmark APIs should be added as product-level How APIs in `but-api` and
  exposed through `but-sdk`, rather than composed from raw Git operations in
  Electron:
  `listBookmarks(projectId)`,
  `createBookmark(projectId, name, source?)`,
  `switchBookmark(projectId, bookmarkId)`,
  `updateBookmark(projectId, bookmarkId)`,
  `renameBookmark(projectId, bookmarkId, name)`, and
  `deleteBookmark(projectId, bookmarkId)`.
- `listBookmarks(projectId)` returns Bookmark ID, display name, target commit
  ID, created/updated timestamps, kind (`user` or `auto`), and `isCurrent`.
  Rust computes `isCurrent` by comparing the active HEAD commit ID with the
  Bookmark target commit ID.
- Bookmark sorting should keep the current Bookmark first, then user-created
  Bookmarks by most recently updated, then auto-preserved Bookmarks by most
  recently updated.
- Bookmark APIs follow the current How boundary: Rust provides atomic
  repository and metadata tools, while Electron remains responsible for
  orchestration, UI status consistency, sequencing, watcher suppression, and
  refreshing Bookmarks/Checkpoints after mutations.
- `switchBookmark(projectId, bookmarkId)` should be one Rust repository mutation
  that validates the Bookmark, moves the internal active line to the Bookmark
  tip, updates the worktree, and fails closed. Electron decides and performs the
  surrounding steps: fast Checkpoint if dirty, backup Bookmark if missing, UI
  status updates, and post-mutation refreshes.
- Fast Checkpoints used during Bookmark switching, Bookmark updating, and
  publish preparation are an Electron policy choice. Electron passes a plain
  timestamp message and does not enqueue async AI summary updates for those
  Checkpoints.
- Normal autosave Checkpoints are created immediately with a timestamp title.
  Electron may enqueue parallel AI summary generation afterwards, but applies
  any resulting message update sequentially through a How API that resolves the
  Checkpoint by explicit Change ID.
- Rust should expose a How-level `updateCheckpointMessageByChangeId` operation
  that validates the visible first-parent Checkpoint chain, skips published
  Checkpoints, preserves the Change ID header, and rebases descendant commits.
- Bookmark implementation requires targeted Rust API tests plus How end-to-end
  tests for create, switch, update, delete, current markers, dirty fast
  Checkpoints, and publish isolation.
- `getGithubPublishStatus(projectId)` with current branch, upstream, GitHub
  destination, and credential readiness as How-level concepts.
- `startGithubDeviceLogin()` / `completeGithubDeviceLogin()` for How-owned OAuth
  device flow credentials.
- `publishToGithub(projectId, options)` with plain failure categories for
  missing branch, missing GitHub destination, rejected shared-project update,
  authentication, permission, and network failures.
- `fetchSharedProject(projectId)` or `refreshSharedProject(projectId)` that
  fetches the configured upstream without mutating the active line.
- `getSharedProjectStatus(projectId)` that reports configured upstream, current
  shared tip, active merge base, whether an update is available, passive fetch
  errors, and whether Publish should be disabled.
- `listUnpublishedCheckpoints(projectId, limit?)`, or equivalent filtering in
  `listCheckpoints`, so the app shows only Checkpoints not reachable from the
  configured shared-project upstream.
- `updateProjectFromShared(projectId)` as a product-level operation that creates
  a pre-update Checkpoint when needed, replays only unpublished active-line work,
  refuses to rewrite published commits, leaves Bookmarks untouched, and fails
  closed with rollback on conflicts or unsupported states.
- `getConfiguredGithubCredential(projectId)` that resolves the credential
  referenced by `how.githubCredential`, without requiring How to list every
  known GitHub account.
- `listGithubProjectDestinations(credentialId)` returning publishable GitHub
  repositories as How-level project destination options.
- `createGithubProjectDestination(projectId, credentialId, name)` that creates
  an empty private GitHub project, adds the HTTPS project destination, and
  configures tracking.
- `getCheckpointStatus(projectId)` or `projectEligibility(projectId)`.
- `meaningfulChanges(projectId)` or a Checkpoint dry-run/status result.

These APIs should return How-level concepts and plain failure categories rather
than making the caller assemble branch, commit, diff, and repository details.

## Open Questions

- What exact implementation should represent Checkpoints: commits, oplog
  snapshots, branches, or a new abstraction?
- What is the precise "meaningful diff" filter for autosave Checkpoints?
- Where should the Checkpoint debouncer live: renderer, Electron main, Rust, or
  a shared service?
- What durable Git-readable marker should record that a Checkpoint was published
  to GitHub?
- How should How support reworking an existing review without exposing branch
  management?
- How should stacked reviews work while preserving the one-current-Change
  product model?
- What plain-language error taxonomy is needed for unsupported states?
- Should we auto-detect file changes that should not be published? Ideally, we snapshot everything as commits, but we don't publish API_KEYs.

## Decision Appendix

- Primary audience: non-technical builders first, normal developers second.
- Primary object: Change.
- V1 scope: one active Change at a time.
- Core loop: open/start project, work, automatic Checkpoints, restore if needed,
  publish when ready.
- Saved versions are called Checkpoints.
- Checkpoints are automatic only in v1.
- Named saved states are called Bookmarks.
- A Bookmark preserves the full Checkpoint chain reachable from its saved state,
  not just a single project snapshot.
- Switching to a Bookmark does not add future Checkpoints to that Bookmark; it
  remains fixed until explicitly updated.
- New Checkpoints after switching do not auto-create backup Bookmarks.
- Switching to a Bookmark preserves the previous state as another Bookmark only
  when that state is not already represented by an existing Bookmark.
- Dirty Bookmark switches create a fast Checkpoint without AI summary
  generation before preserving and switching.
- Bookmark switching fails closed: if the switch cannot complete, the user stays
  where they are and any already-created backup Bookmark remains.
- Auto-preserved Bookmarks are visible in the normal Bookmark list with a clear
  system-generated name.
- The current Bookmark remains visible in the Bookmark list and is marked as
  current.
- Bookmarks live in a project sidebar; the Checkpoint timeline remains the main
  interaction center.
- Empty Bookmark sidebars stay compact and action-oriented.
- Bookmark search/filtering is not part of the first implementation.
- Users can delete Bookmarks with confirmation; deleting a Bookmark removes the
  pointer, not the underlying Checkpoint commits.
- Deleting the current Bookmark leaves the current project state unchanged.
- Users can rename Bookmarks; rename updates metadata only.
- Bookmarks are local-only in the first implementation and are not published or
  shared.
- Publishing ignores Bookmarks and operates only on the active project state.
- Internally, Bookmarks use private refs outside `refs/heads`, and How keeps
  `main` as the single active working line that moves to the selected Bookmark
  tip when switching.
- Bookmarks have stable internal IDs separate from friendly display names.
- Bookmark metadata is local JSON under `.git/gitbutler` for now, with a future
  path to git-meta.
- Rust Bookmark APIs are atomic tools; Electron remains the consistency and
  orchestration layer.
- Bookmark listings include `isCurrent` from Rust and sort current first, then
  user Bookmarks, then auto-preserved Bookmarks.
- Bookmark display names may duplicate; stable IDs define identity and the UI
  should nudge toward clear names.
- Renaming or manually updating an auto-preserved Bookmark promotes it to a user
  Bookmark.
- Users can explicitly Bookmark the current state; unsaved changes are saved as
  a Checkpoint before the Bookmark is created.
- How does not create a default Bookmark automatically on project open/start.
- Bookmark representation checks compare commit IDs only.
- Browsing mode can create a new Bookmark from a clean browsed Checkpoint, but
  cannot update existing Bookmarks or bookmark dirty browsing edits.
- Users can explicitly update any existing Bookmark to the current state without
  switching to it first; this is a confirmed destructive replacement and does
  not auto-preserve the old Bookmark target.
- Bookmark behavior should be covered by targeted Rust/API tests and How e2e
  tests.
- Watcher-driven Checkpoints use a 10-second quiet period and meaningful-diff
  filter.
- Generation-completion hooks can create Checkpoints immediately.
- Timeline is time-based, with optional AI labels.
- AI Checkpoint summaries use the configured coding agent, the saved Checkpoint
  patch, async best-effort generation, and a timestamp fallback.
- No diff viewer in v1.
- Restore uses browsing mode: viewing Checkpoints pauses autosave and does not
  choose a new direction until the user clicks **Continue from here**.
- Browsing state is persisted locally for now. Design a Git-readable
  forward/back history later.
- Publish has one V1 flow: log in to GitHub, choose or create a GitHub project,
  then publish.
- There is no publish mode choice in V1.
- Publishing updates the shared project; avoid `main` language in the UI.
- Publishing can create an empty private GitHub project destination under the
  authenticated GitHub user when no destination exists.
- GitHub-created destinations use editable folder-derived names, HTTPS URLs, and
  no silent auto-suffixing.
- How's first GitHub setup path is OAuth device flow login from the Publish
  flow. How does not use `listKnownGithubAccounts` or show an account picker in
  the first implementation.
- How stores GitHub OAuth credentials in How-owned secure credential storage,
  separate from GitButler's existing GitHub account storage.
- How stores only an opaque credential reference per project as
  `how.githubCredential`; credentials never live in Git config.
- Publish creates a Checkpoint first.
- Review publishing is future work, including GitHub Enterprise, forks, draft
  reviews, rework, and stacked reviews.
- Successful publish marks the pre-publish Checkpoint with the outcome.
- After successful Publish, How refreshes upstream knowledge and hides
  Checkpoints that are now part of the shared project.
- The Checkpoint timeline shows unpublished local work only.
- How fetches the configured shared-project upstream continuously, every 15
  minutes by default, with a per-project **Check for shared updates** setting.
- Background fetch is read-only and never mutates the active line.
- When the active line is behind the shared project, How shows **Update
  available**, offers **Update project**, and disables Publish with a tooltip.
- The actual **Update project** operation is a near-future gap, not part of the
  current MVP implementation.
- No force-push, rebase choices, or conflict-resolution UI in v1.
- No advanced mode and no Lite escape hatch in v1.
- Errors are plain-language only.
- Main screen is one calm Change screen.
- Complex existing project states are unsupported in v1.
- Brand-new non-Git folders are eligible.
- First run only offers open existing project or start new project.
- Start new project does not scaffold an app.
- No Change naming in v1.
- AI labels are optional and non-blocking.
- Missing publish setup is asked at Publish time.
- V1 supports GitHub only.
- Publish setup language is "Log in to GitHub", "Create GitHub project", and
  "Choose existing project".
- Publish success language is "Published to the shared project".
- Users cannot manage Checkpoints in v1.
