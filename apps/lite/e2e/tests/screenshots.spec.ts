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

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { Page } from "@playwright/test";
import { enabled, goToTab, openProject, outputDir, shoot } from "../screenshot-helpers.ts";
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

		test("pull request form", async ({ appWindow }) => {
			await openProject(appWindow);
			// The branch tab is Redux state, not a route, so like the commit form
			// this surface depends on a click producing a frame.
			await appWindow.getByRole("treeitem", { name: "C", exact: true }).click();
			await appWindow.getByRole("button", { name: "Pull Request" }).click();
			await expect(appWindow.getByPlaceholder("PR title")).toBeVisible();
			await shoot(appWindow, "pr-form", "#details-panel");
		});
	});

	test.describe("remote branches", () => {
		test.use({ scenario: "project-with-remote-branches.sh" });

		test("branches tab", async ({ appWindow }) => {
			await openProject(appWindow);
			await goToTab(appWindow, "branches");
			await shoot(appWindow, "branches-tab", "#sidebar-panel");
		});

		test("upstream tab", async ({ appWindow }) => {
			await openProject(appWindow);
			await goToTab(appWindow, "upstream");
			await shoot(appWindow, "upstream-tab", "#sidebar-panel");
		});

		test("project picker", async ({ appWindow }) => {
			await openProject(appWindow);
			await appWindow.getByRole("button", { name: /select project/i }).click();
			await shoot(appWindow, "project-picker", '[class*="PickerDialog-module_popup"]');
		});
	});

	test.describe("uncommitted changes", () => {
		// The dirty-worktree fixtures don't register a project, so reuse a scenario
		// that does and make the working tree dirty here instead.
		test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

		/** Leaves the seeded clone with one added and one modified file. */
		const dirty = async (appWindow: Page, workdir: string): Promise<void> => {
			const clone = path.join(workdir, "local-clone");
			writeFileSync(path.join(clone, "added.txt"), "a new, uncommitted file\n");
			writeFileSync(path.join(clone, "base.txt"), "base, now modified\n");
			await appWindow.reload();
			await appWindow.getByRole("main").waitFor();
			// Wait for the rows themselves. Without this both runs can photograph an
			// empty list, match, and be reported as "unchanged" — a surface silently
			// contributing nothing while looking like coverage.
			await expect(appWindow.getByText("added.txt")).toBeVisible();
		};

		test("uncommitted file rows", async ({ appWindow, testEnvironment }) => {
			await openProject(appWindow);
			await dirty(appWindow, testEnvironment.workdir);
			await shoot(appWindow, "uncommitted", "#sidebar-panel");
		});

		test("commit form", async ({ appWindow, testEnvironment }) => {
			await openProject(appWindow);
			await dirty(appWindow, testEnvironment.workdir);
			// Expanding the form is in-app state, not a navigation, so this surface
			// depends on a click producing a frame the way the dialogs do.
			await appWindow.getByRole("button", { name: /start commit/i }).click();
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
