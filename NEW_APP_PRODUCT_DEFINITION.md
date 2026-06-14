# How Product Definition

Status: consolidated living draft

## One-Line Definition

How is not a simplified Git client. It is a way to manage changes while
building software.

## Manifesto

Git is infrastructure, not the product surface.

How is for people who are building software but should not need Git concepts
to keep working, return to earlier moments, or publish when they are ready. The
user should think in terms of Changes, Checkpoints, publishing, and the shared
project. Branches, commits, stacks, rebases, force-pushes, remotes, and conflict
machinery are implementation details.

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

**Publish**:
The act of sending the current Change to the project destination. V1 supports
two publish modes: review before publishing, and publish directly.

**Project destination**:
The place the project is published to. Use this instead of "remote" in the
product surface.

**Shared project**:
The published project state updated by direct publish. Use this instead of
`main` or `origin/main` in the product surface.

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
  changes", or "Could not save".
- Time-based Checkpoint timeline, with an AI-generated label when available.
- Primary action: Publish.
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
- The summary input is the staged Git diff for the Checkpoint being created.
- The diff payload is capped at roughly 40 KB. If truncated, the prompt tells
  the agent to summarize only the visible diff.
- Summary generation is read-only. The agent should not edit files or inspect
  the project beyond the diff How provides.
- How waits briefly, currently 2 seconds, then falls back to the timestamp
  title.
- Once How falls back, it does not amend the Checkpoint later.
- Summary failures are silent in the UI when the fallback Checkpoint succeeds.
  Logs should include the selected agent, diff size, truncation state, timeout,
  and fallback reason, but not the full diff or generated summary.

Agent output is strict plain text. The first line is the title. Remaining text
is the optional body. How owns the `Checkpoint:` prefix, strips an accidental
agent-provided `Checkpoint:` prefix from the title, and stores the body only in
the Git commit message body.

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

AI labels may briefly delay Checkpoint creation, but they must fall back quickly
and never prevent saving, restoring, or publishing. The app should not require
account or AI setup before it is useful.

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

V1 supports two publish modes:

1. **Review before publishing**:
   Creates a review on GitHub against the project's trunk.
2. **Publish directly**:
   Updates the shared project without review.

Use GitHub only in v1.

The app asks the user to choose the publish mode the first time they click
Publish, not during initial project setup. The choice becomes sticky project
configuration and is reused for future publishes. The user can change it later
in settings.

Publish always creates a Checkpoint immediately before doing any update or
publish work. This captures the exact state the user intended to publish, even
if the quiet-period autosave had not fired yet.

After publishing succeeds, mark the pre-publish Checkpoint with the outcome,
such as "Published" or "Review created". Do not create a separate post-publish
Checkpoint unless files actually changed during the update/publish flow.

MVP direct publish does not yet create a durable timeline marker or amend the
pre-publish Checkpoint with the publish outcome. A successful publish updates
the status text to "Published just now" only. Durable publish markers remain a
future Git-readable design problem.

## Publish Setup

Missing publish setup is handled just-in-time when the user clicks Publish.

For review workflow, the app may need to connect GitHub, choose or create a
project destination, and create a review against trunk.

For direct publish, the app may need to choose or create a project destination,
then publish directly to the configured shared project.

Use plain language:

- "Project destination", not "remote".
- "Review before publishing", not "pull request" or "PR" as the primary label.
- "Created a review on GitHub", not "opened a PR" as the primary result.
- "Publish directly" and "updates the shared project without a review".
- "Shared project", not `main` or `origin/main`.

## Direct Publish

Direct publish is available only for projects explicitly configured for a
non-review/direct-publish workflow. The project configuration decision is made
the first time the user clicks Publish, not during first run.

The first-time Publish dialog asks "How should this project publish?" It shows:

- **Publish directly**: enabled. "Updates the shared project without a review."
- **Review before publishing**: disabled placeholder. "Coming later."

Choosing direct publish stores only `how.publishMode = direct` in local Git
config. How does not duplicate destination URL or branch tracking information in
How-specific config. Normal Git remote and upstream configuration remain the
source of truth for where the project publishes.

MVP direct publish destination rules:

1. If the current branch already has a remote/upstream, push to that destination.
2. If the current branch has no upstream but the repository has remotes, publish
   to a remote branch with the same name as the local branch and set upstream
   tracking. Prefer `origin` when it exists; otherwise use the first remote.
3. If the repository has no remote, ask for a project destination URL, add it as
   `origin`, then publish the current branch to `origin/<current branch>` and
   set upstream tracking.

Direct publish creates a Checkpoint before pushing only when there are unsaved
changes. If that Checkpoint cannot be created, publishing stops. If pushing
fails, the Checkpoint remains because it captured the intended publish state.

Direct publish never force-pushes. It does not automatically pull, merge,
rebase, or update in the MVP. If the shared project has changed and rejects the
push, How shows a plain-language error and leaves the local project unchanged.

Publishing is disabled while browsing Checkpoints. The user must choose
**Continue from here** or **Return to latest** before publishing.

## Update Behavior

Updating from the shared/trunk version is mostly automatic and publish-driven.

Before publishing, the app checks whether the shared version has changed. If it
has, the app creates a "Before update" Checkpoint and tries to update
automatically. If update succeeds, publishing continues.

Outside publishing, the app may show that the shared version has updates and
offer one simple action: Update project.

If update conflicts or otherwise requires advanced intervention, v1 stops
safely with a plain-language error.

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
- `getDirectPublishStatus(projectId)` with current branch, upstream, and
  destination setup state as How-level concepts.
- `configureDirectPublish(projectId, destinationUrl?)`.
- `publishDirect(projectId, options)` with plain failure categories for missing
  branch, missing destination, rejected shared-project update, authentication,
  and network failures.
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
- What does the project settings surface look like for changing publish mode
  without becoming an advanced mode?
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
- Watcher-driven Checkpoints use a 10-second quiet period and meaningful-diff
  filter.
- Generation-completion hooks can create Checkpoints immediately.
- Timeline is time-based, with optional AI labels.
- AI Checkpoint summaries use the configured coding agent, staged diff only, a
  2-second timeout, and a timestamp fallback.
- No diff viewer in v1.
- Restore uses browsing mode: viewing Checkpoints pauses autosave and does not
  choose a new direction until the user clicks **Continue from here**.
- Browsing state is persisted locally for now. Design a Git-readable
  forward/back history later.
- Publish supports review and direct modes.
- Publish mode is chosen at first Publish and sticks per project.
- Direct publish is allowed only for projects configured for direct publish.
- Direct publish updates the shared project; avoid `main` language in the UI.
- Publish creates a Checkpoint first.
- Successful publish marks the pre-publish Checkpoint with the outcome.
- Updates before publish are automatic when possible.
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
- Review workflow language is "Review before publishing" and "Created a review
  on GitHub".
- Direct publish language is "Publish directly" and "Published to the shared
  project".
- Users cannot manage Checkpoints in v1.
