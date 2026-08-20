---
name: lite-screenshots
description: Use when asked to add before/after screenshots of a Lite UI change to a pull request, when a PR touching apps/lite/ui needs its visual change shown, or when extending the screenshot catalogue in apps/lite/e2e/tests/screenshots.spec.ts. Captures both sides locally against seeded fixtures, publishes the surfaces that changed, and posts them to the PR.
---

A reviewer reading a CSS diff cannot see the result. This captures the Lite UI
with the branch applied and again with it unapplied, and posts the surfaces that
differ to the pull request.

Capture runs **on this machine**, not in CI. It was tried in CI and abandoned:
under the bare X server on the Linux runners this Electron build paints a frame
on load and not reliably afterwards, so several surfaces hung indefinitely
whether the capture went through Playwright or the DevTools protocol. The same
spec passes here. The CI workflow only leaves a reminder.

## Before capturing: is the surface covered?

Work this out first, by reading the diff — not by capturing and inferring it from
the result. A run that reports "8 surfaces, 0 changed" looks identical whether the
change is invisible or whether nothing looked at it.

1. Identify which screen the change affects.
2. Check it against the catalogue in `apps/lite/e2e/tests/screenshots.spec.ts`
   (currently: workspace sidebar, diff pane, branches tab, upstream tab, project
   picker, uncommitted rows, commit form, settings).
3. **Covered** — capture, as below.
4. **Not covered** — stop and ask the developer, offering the two lanes below.
   Either way it is their call, not one to make quietly inside a screenshot
   request. Tell them which screen is uncovered and which fixture would reach it.
   - **Add it to the catalogue** when the gap is a _screen_ — something anyone
     working nearby will change again. It edits a shared file and becomes
     permanent coverage for every later pull request. See "Extending the
     catalogue" below.
   - **Capture it ad hoc** when the gap is a _state_ — a conflicted commit
     mid-resolution, an empty list, one specific dialog — that no future pull
     request is likely to want. Nothing shared changes and nothing is left
     behind. See "One-off capture" below.

   Prefer the catalogue when it is a close call. The ad-hoc lane photographs the
   change once and protects nothing afterwards, and a catalogue that stops growing
   makes every later request re-derive the same fixtures.

If they decline both, **do not post a comment**. A normal comment would report
"N surfaces, 0 changed", which asserts the change is not visual when in truth it
was never photographed. Say plainly that the affected screen is not in the
catalogue and no screenshots were taken.

## Changes the capture cannot see

Some diffs are invisible to this harness for reasons that have nothing to do with
the change being non-visual. Rule these out before reading a "0 changed" result
as an answer:

- **Scrollbars and their gutter.** macOS hides scrollbars unless a mouse is
  attached, and a styled `::-webkit-scrollbar` does _not_ override that — the
  scroller simply reserves no gutter. On a machine set to "When scrolling", any
  change to the track, thumb, gutter width, or a separator meeting the gutter
  captures identically on both sides. Set System Settings → Appearance → Show
  scroll bars → **Always** for the run, and say in the comment that you did.
- **Hover, focus, and drag states** are not entered by the spec.
- **Anything behind a scroll position** — surfaces are captured at the top.

## Prerequisites

- `cargo build -p but` once, giving `target/debug/but`.
- The branch under test applied, and **not yet merged** — the base side comes
  from unapplying it. Check `but status` first.
- **Run every capture through `env -u ELECTRON_RUN_AS_NODE`.** An agent hosted
  inside VS Code (itself Electron) inherits `ELECTRON_RUN_AS_NODE=1`, which makes
  the Electron the test launches behave as plain Node. Every surface then fails
  with `Error: Process failed to launch!` and nothing says why — the binary is
  fine, and running it by hand from another shell works. Check with
  `env | grep ELECTRON`.

## The flow

Run every command from the repository root.

### 1. Capture the branch as it stands

```console
$ env -u ELECTRON_RUN_AS_NODE SCREENSHOT_OUT=head BUT=$PWD/target/debug/but \
    pnpm -F @gitbutler/lite test:e2e screenshots.spec.ts
```

Output lands in `apps/lite/e2e/screenshots/head/` (gitignored).

### 2. Capture the base

```console
$ but unapply <branch>
$ but status        # confirm the branch is gone from the workspace
$ env -u ELECTRON_RUN_AS_NODE SCREENSHOT_OUT=base BUT=$PWD/target/debug/but \
    pnpm -F @gitbutler/lite test:e2e screenshots.spec.ts
$ but apply <branch>
```

**Check the unapply took effect.** It silently does nothing on a stale id or an
already-merged branch, and both runs then capture identical code — which looks
exactly like "this change is not visual". If every pair matches in step 3, assume
this went wrong before concluding anything about the change.

Re-apply immediately, before anything else can fail and leave the workspace short
a branch.

**Stacked branches take the whole stack down.** `but unapply` on a branch that is
part of a stack unapplies every branch in it, including ones this capture depends
on — unapplying a stacked change also removes the spec, and the base run then has
no tests to execute. Check `but status` first. When the branch is stacked, capture
the base by reverting the change in the working tree instead:

1. Edit the changed files back to their pre-change state by hand.
2. Capture into `base`.
3. `but discard <file-id>` to restore the committed version.

Confirm the restore: the change must be back before anything else happens.

### 3. Select what changed

```console
$ node apps/lite/e2e/compare-screenshots.mjs \
    apps/lite/e2e/screenshots/base \
    apps/lite/e2e/screenshots/head \
    /tmp/publish \
    "https://raw.githubusercontent.com/<owner>/<repo>/pr-screenshots/pr-<number>/<short-sha>" \
    /tmp/section.md
```

It prints `total=`, `changed=`, `added=`, stages only the differing pairs into
`/tmp/publish`, and writes the comment body. Surfaces that did not change are
byte-identical, so they are folded into a collapsed list rather than shown.

### 4. Publish the images

Images go on an orphan `pr-screenshots` branch, under `pr-<number>/<short-sha>/`.
A per-commit directory matters: reusing one path lets GitHub's image proxy serve
the previous run's screenshots from cache, which reads as "my change did nothing".

Write them with the git data API rather than a local clone. There is nothing to
clone, nothing to clean up, and no local `git` writes — which agents are often
not permitted to make outside the project, and which cannot touch the GitButler
workspace by accident.

```console
$ REPO=<owner>/<repo>; DIR=pr-<number>/<short-sha>
$ HEAD_SHA=$(gh api repos/$REPO/git/refs/heads/pr-screenshots --jq '.object.sha')
$ BASE_TREE=$(gh api repos/$REPO/git/commits/$HEAD_SHA --jq '.tree.sha')

# One blob per image. base64 must be stripped of newlines.
$ jq -n --rawfile c <(base64 -i /tmp/publish/<name>.png) \
      '{content: ($c | gsub("\n";"")), encoding: "base64"}' > /tmp/blob.json
$ BLOB=$(gh api -X POST repos/$REPO/git/blobs --input /tmp/blob.json --jq '.sha')

# One tree and one commit for all of them, then move the ref.
$ jq -n --arg t "$BASE_TREE" --arg b "$BLOB" --arg d "$DIR" '{base_tree: $t, tree: [
      {path: ($d + "/<name>.png"), mode: "100644", type: "blob", sha: $b}]}' > /tmp/tree.json
$ TREE=$(gh api -X POST repos/$REPO/git/trees --input /tmp/tree.json --jq '.sha')
$ jq -n --arg t "$TREE" --arg p "$HEAD_SHA" \
      '{message: "Screenshots for #<number>", tree: $t, parents: [$p]}' > /tmp/commit.json
$ COMMIT=$(gh api -X POST repos/$REPO/git/commits --input /tmp/commit.json --jq '.sha')
$ gh api -X PATCH repos/$REPO/git/refs/heads/pr-screenshots -f sha="$COMMIT"
```

When the branch does not exist yet, omit `base_tree` and `parents`, then create
the ref with `POST git/refs -f ref=refs/heads/pr-screenshots -f sha=$COMMIT`.

**Fetch one raw URL before posting.** A comment full of broken images is worse
than no comment, and the failure is invisible until someone opens the pull
request:

```console
$ curl -s -o /dev/null -w '%{http_code}\n' \
    https://raw.githubusercontent.com/<owner>/<repo>/pr-screenshots/<dir>/<name>.png
```

### 5. Post to the pull request

Upsert a single comment rather than stacking one per run — find a previous
comment starting with `<!-- lite-screenshots -->` and PATCH it, else POST.

```console
$ jq -Rs '{body: .}' /tmp/section.md > /tmp/payload.json
$ gh api "repos/<owner>/<repo>/issues/<number>/comments" --paginate \
    --jq 'map(select(.body | startswith("<!-- lite-screenshots -->"))) | first | .id // empty'
$ gh api -X PATCH "repos/<owner>/<repo>/issues/comments/<id>" --input /tmp/payload.json
```

Do not edit the pull request description: rewriting text a human owns risks
clobbering their concurrent edits.

### 6. Swap the label

`screenshots needed` is the request; `screenshots` is the record. Move the pull
request from one to the other once the comment is posted, so the label says what
is true:

```console
$ gh pr edit <number> --remove-label "screenshots needed" --add-label "screenshots"
```

Only after a comment with images actually went up. A pull request labelled
`screenshots` with none attached is worse than an unlabelled one — it tells a
reviewer the change has been shown when it has not. If you declined to post (an
uncovered surface, a capture that failed), leave `screenshots needed` where it is.

Seeing both labels later is expected, not a bug to tidy away: the labeler re-adds
`screenshots needed` on the next push, which says a set exists and the code has
moved since. Recapturing swaps it away again — the command above is idempotent.

If you ever post images that are **not** captures of the running app — a page
rendered from the branch's CSS, say, when the harness cannot be run — label them
as such in the comment and mark it `<!-- lite-screenshots-replica -->` instead.
A reviewer assumes a screenshot is a photograph of the app; anything else has to
say so, and the different marker keeps a later real run from overwriting it.

## Before finishing

- **Look at the images**, do not just trust exit codes. A green run proves files
  were written, not that they show the surface.
- Confirm the workspace is whole: `but status` should show the branch applied and
  no stray uncommitted changes.

## One-off capture

For a state the catalogue should not carry permanently. The run command filters
by filename, so any spec whose name contains `screenshots` is picked up by the
same command and writes into the same `SCREENSHOT_OUT` directory — the compare
step cannot tell the difference.

Write `apps/lite/e2e/tests/screenshots-adhoc.spec.ts`, importing the shared
helpers rather than copying them (every workaround in them was earned, and a copy
loses the comments explaining why):

```ts
import { enabled, openProject, shoot } from "../screenshot-helpers.ts";
import { test } from "../test.ts";

test.describe("screenshots", () => {
	test.skip(!enabled, "set SCREENSHOT_OUT to capture screenshots");
	test.describe.configure({ timeout: 180_000 });
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("<surface>", async ({ appWindow }) => {
		await openProject(appWindow);
		// ...reach the state...
		await shoot(appWindow, "<surface-name>", "<selector>");
	});
});
```

The same fixture and selector rules as the catalogue apply — see below.

Then capture both sides as usual, and:

- **Delete the file** once the base run is done. It is untracked and not ignored,
  so left behind it shows up as a stray change in `but status` and eventually in
  someone's commit. The "confirm the workspace is whole" check at the end is what
  catches this.
- **Say in the comment** that the surface was captured ad hoc and is not covered
  going forward, so nobody reads it as a regression guarantee.

**Never run the two specs in one command.** The config sets `fullyParallel`, and
the catalogue spec wipes `SCREENSHOT_OUT` in a `beforeAll` — run concurrently, that
wipe lands in the middle of the ad-hoc capture and deletes it. What survives is a
plausible-looking directory missing exactly the surface you were asked for.

The trailing argument is a **substring match on the file path**, so a bare
`screenshots` selects the ad-hoc spec as well and puts you straight into that race.
Name the file: `screenshots.spec.ts` for the catalogue, `screenshots-adhoc` for the
ad-hoc spec. Steps 1 and 2 above already do.

Capture the ad-hoc surface **alone**, filtering to its filename, into its own
output directory on each side:

```console
$ env -u ELECTRON_RUN_AS_NODE SCREENSHOT_OUT=adhoc-head BUT=$PWD/target/debug/but \
    pnpm -F @gitbutler/lite test:e2e screenshots-adhoc
```

Its own directory also avoids inheriting stale PNGs from an earlier catalogue run,
since only the catalogue spec cleans up after itself. Then compare
`adhoc-base` against `adhoc-head` as in step 3.

When a change touches both a covered screen and an uncovered state, run the
catalogue first and the ad-hoc spec second, in separate commands, pointing both at
the same directory — that order is safe because only the first one wipes.

## Extending the catalogue

Only after the developer has agreed (see the top of this document). Add the
surface to `apps/lite/e2e/tests/screenshots.spec.ts`, then capture as usual —
the same run proves the new surface works and shows the change. Coverage applies
to every later pull request, not just this one.

```ts
test.describe("<area>", () => {
	test.use({ scenario: "<fixture>.sh" });

	test("<surface>", async ({ appWindow }) => {
		await openProject(appWindow);
		await goToTab(appWindow, "branches"); // when it lives on another tab
		await shoot(appWindow, "<surface-name>", "<selector>");
	});
});
```

- `<surface-name>` becomes the filename and the heading a reviewer reads: name it
  `commit-form`, not `outline-panel-2`.
- Only fixtures calling `"$BUT" setup` register a project; the rest leave the app
  on "Select a project." and every capture fails. Known good:
  `project-in-single-branch-three-branch-stack.sh`,
  `project-with-remote-branches.sh`, `project-with-conflicting-commits.sh`.
- If no fixture reaches the state, build it in the test — `uncommitted` writes
  files into `testEnvironment.workdir/local-clone` and reloads.
- Prefer a stable id for the clip (`#outline-panel`, `#details-panel`), else a
  CSS-module prefix or an ARIA hook. Never nth-child or a generated class.
- One screenshot per test, taken after a fresh load. This costs nothing here and
  keeps the spec usable if CI capture is ever revisited.

## Out of scope

- **Native context menus** cannot be captured; they are OS windows.
- **Dark mode** is not covered — the harness seeds `theme: "light"`.
- **States needing a real remote or credentials** cannot be fixtured.
