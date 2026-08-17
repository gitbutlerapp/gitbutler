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
4. **Not covered** — stop and ask the developer. Adding a surface edits a shared
   file and changes what every later pull request captures, so it is their call,
   not one to make quietly inside a screenshot request. Tell them which screen is
   uncovered, which fixture would reach it, and that it becomes permanent coverage
   for everyone.

If they decline, **do not post a comment**. A normal comment would report
"N surfaces, 0 changed", which asserts the change is not visual when in truth it
was never photographed. Say plainly that the affected screen is not in the
catalogue and no screenshots were taken.

## Prerequisites

- `cargo build -p but` once, giving `target/debug/but`.
- The branch under test applied, and **not yet merged** — the base side comes
  from unapplying it. Check `but status` first.

## The flow

Run every command from the repository root.

### 1. Capture the branch as it stands

```console
$ SCREENSHOT_OUT=head BUT=$PWD/target/debug/but pnpm -F @gitbutler/lite test:e2e screenshots
```

Output lands in `apps/lite/e2e/screenshots/head/` (gitignored).

### 2. Capture the base

```console
$ but unapply <branch>
$ but status        # confirm the branch is gone from the workspace
$ SCREENSHOT_OUT=base BUT=$PWD/target/debug/but pnpm -F @gitbutler/lite test:e2e screenshots
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

```console
$ rm -rf /tmp/pub && mkdir /tmp/pub && cd /tmp/pub
$ git init -q -b pr-screenshots && git remote add origin <repo-url>
$ git fetch -q --depth 1 origin pr-screenshots && git reset -q --hard origin/pr-screenshots
$ mkdir -p pr-<number>/<short-sha> && cp -R /tmp/publish/. pr-<number>/<short-sha>/
$ git add -A && git commit -q -m "Screenshots for #<number>" && git push -q origin pr-screenshots
```

The branch may not exist yet; skip the fetch and reset when it does not.

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

## Before finishing

- **Look at the images**, do not just trust exit codes. A green run proves files
  were written, not that they show the surface.
- Confirm the workspace is whole: `but status` should show the branch applied and
  no stray uncommitted changes.

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
