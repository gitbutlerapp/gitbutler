// Surface catalogue for before/after PR screenshots.
//
// Opt-in: this spec is skipped unless SCREENSHOT_OUT names an output directory,
// so ordinary `test:e2e` runs (including CI) are unaffected.
//
//   SCREENSHOT_OUT=after pnpm -F @gitbutler/lite test:e2e screenshots.spec.ts
//
// Every surface is captured on every run. Deciding which ones actually changed
// is the caller's job: pairs that are byte-identical between two runs did not
// change, and only the rest are worth showing a reviewer.

import { mkdirSync, rmSync } from "node:fs";
import type { App } from "../app.ts";
import { mergeReadiness } from "../merge-readiness-fixture.ts";
import { enabled, goToTab, openProject, outputDir, shoot } from "../screenshot-helpers.ts";
import type { Repo } from "../repo.ts";
import { expect, test } from "../test.ts";

test.describe("screenshots", () => {
	test.skip(!enabled, "set SCREENSHOT_OUT to capture screenshots");
	// Each test launches Electron, seeds a repository with the `but` binary, and
	// waits for the app to load before its one screenshot; the 30s default is not
	// enough for that.
	test.describe.configure({ timeout: 180_000 });
	test.beforeAll(() => {
		// Start empty: a PNG left by an earlier run would survive a rerun in which
		// its surface failed, and be compared as though this run had produced it.
		rmSync(outputDir, { force: true, recursive: true });
		mkdirSync(outputDir, { recursive: true });
	});

	test.describe("stack", () => {
		test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

		test("workspace sidebar", async ({ appWindow }) => {
			await openProject(appWindow);
			await shoot(appWindow, "workspace-sidebar", "#sidebar-panel");
		});

		test("diff pane", async ({ appWindow }) => {
			await openProject(appWindow);
			await shoot(appWindow, "details-pane", "#details-panel");
		});

		test("pull request form", async ({ app, appWindow }) => {
			await openProject(appWindow);
			// The branch tab is Redux state, not a route, so like the commit form
			// this surface depends on a click producing a frame.
			await app.selectBranch("C");
			await app.branchTab("Pull Request").click();
			await expect(app.pullRequestForm).toBeVisible();
			await shoot(appWindow, "pr-form", "#details-panel");
		});

		test.describe("with a pull request", () => {
			// The forge gives the panel reviewers, checks and labels to show.
			test.use({ forge: mergeReadiness() });

			test("pull request panel", async ({ app, appWindow }) => {
				await openProject(appWindow);
				// A branch with an open review opens on its pull request.
				await app.selectBranch("C");
				await expect(
					appWindow.getByRole("heading", {
						name: "Improve keyboard navigation in large repositories",
					}),
				).toBeVisible();
				// Both sides settle on the merge status; the checklist also loads its to-dos.
				await expect(app.mergeButton).toBeDisabled();
				await expect(appWindow.getByText(/Checking mergeability|Loading to-dos/)).toHaveCount(0);
				await shoot(appWindow, "pr-panel", "#details-panel");
			});
		});
	});

	test.describe("edit mode", () => {
		test.use({ scenario: "project-with-conflicted-commit.sh" });

		test("edit mode page", async ({ app, appWindow }) => {
			await openProject(appWindow);

			// Edit mode is repository state rather than a route, so it is entered
			// through the same call the commit menu makes and then reloaded into:
			// the surface has to be reached by a load, never by a click.
			await app.enterEditMode();
			await app.reload();
			await expect(app.editModeHeading).toBeVisible();

			// The page centres a 640px column in the whole outlet, so clipping the
			// outlet would be mostly empty background: clip the column itself.
			await shoot(appWindow, "edit-mode", '[class*="EditModePage-module_panel"]');
		});
	});

	test.describe("remote branches", () => {
		test.use({ scenario: "project-with-remote-branches.sh" });

		test("branches tab", async ({ appWindow }) => {
			await openProject(appWindow);
			await goToTab(appWindow, "branches");
			await shoot(appWindow, "branches-tab", "#sidebar-panel");
		});

		test("project picker", async ({ app, appWindow }) => {
			await openProject(appWindow);
			await app.projectPicker.click();
			await shoot(appWindow, "project-picker", '[class*="ProjectPicker-module_popup"]');
		});
	});

	test.describe("uncommitted changes", () => {
		// The dirty-worktree fixtures don't register a project, so reuse a scenario
		// that does and make the working tree dirty here instead.
		test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

		/** Leaves the seeded clone with one added and one modified file. */
		const dirty = async (app: App, repo: Repo): Promise<void> => {
			await repo.write("added.txt", "a new, uncommitted file\n");
			await repo.write("base.txt", "base, now modified\n");
			// Wait for the rows themselves. Without this both runs can photograph an
			// empty list, match, and be reported as "unchanged" — a surface silently
			// contributing nothing while looking like coverage.
			await expect(app.uncommittedFile("added.txt")).toBeVisible();
		};

		test("uncommitted file rows", async ({ app, appWindow, repo }) => {
			await openProject(appWindow);
			await dirty(app, repo);
			await shoot(appWindow, "uncommitted", "#sidebar-panel");
		});

		test("commit form", async ({ app, appWindow, repo }) => {
			await openProject(appWindow);
			await dirty(app, repo);
			// Expanding the form is in-app state, not a navigation, so this surface
			// depends on a click producing a frame the way the dialogs do.
			await app.startCommit();
			await shoot(appWindow, "commit-form", "#sidebar-panel");
		});
	});

	test.describe("settings", () => {
		test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

		test("settings dialog", async ({ appWindow }) => {
			await openProject(appWindow);
			// Settings is held in Redux rather than the URL, so it can only be opened
			// by clicking — as with the project picker.
			await appWindow.getByRole("button", { name: "Settings" }).click();
			await shoot(appWindow, "settings", '[aria-labelledby="settings-heading"]');
		});
	});
});
