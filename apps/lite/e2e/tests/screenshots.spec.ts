// Surface catalogue for before/after PR screenshots.
//
// Opt-in: this spec is skipped unless SCREENSHOT_OUT names an output directory,
// so ordinary `test:e2e` runs (including CI) are unaffected.
//
//   SCREENSHOT_OUT=after pnpm -F @gitbutler/lite test:e2e screenshots
//
// Every surface is captured on every run. Deciding which ones actually changed
// is the caller's job: pairs that are byte-identical between two runs did not
// change, and only the rest are worth showing a reviewer.

import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import type { Page } from "@playwright/test";
import { expect, test } from "../test.ts";

const out = process.env.SCREENSHOT_OUT;
const enabled = out !== undefined && out !== "";

// A wide-but-ordinary window, so panes are laid out the way a reviewer sees them.
const VIEWPORT = { width: 1440, height: 900 };

const outputDir = path.resolve(import.meta.dirname, "../screenshots", out ?? "unused");

const shoot = async (appWindow: Page, name: string, selector: string): Promise<void> => {
	const target = appWindow.locator(selector);
	await expect(target).toBeVisible();
	// Let transitions and any late layout settle before the shutter.
	await appWindow.waitForTimeout(400);

	const box = await target.boundingBox();
	if (box === null) throw new Error(`${name}: ${selector} has no bounding box`);

	// Capture over the DevTools protocol rather than through page.screenshot().
	// Playwright waits for the compositor to hand it a fresh frame, and under a
	// bare X server this renderer only produces one on load, so that wait never
	// returns for a surface reached any other way. CDP takes what is on screen.
	const session = await appWindow.context().newCDPSession(appWindow);
	try {
		const shot = (await session.send("Page.captureScreenshot", {
			format: "png",
			clip: { x: box.x, y: box.y, width: box.width, height: box.height, scale: 1 },
		})) as { data: string };
		writeFileSync(path.join(outputDir, `${name}.png`), Buffer.from(shot.data, "base64"));
	} finally {
		await session.detach();
	}
};

const openProject = async (appWindow: Page): Promise<void> => {
	await expect(appWindow).toHaveURL(/\/project\/[^/]+\/workspace/);
	await expect(appWindow.getByRole("button", { name: /select project/i })).toBeVisible();
	await appWindow.setViewportSize(VIEWPORT);

	// On CI the renderer the window starts with sometimes never produces a frame
	// ("sandboxed_renderer.bundle.js script failed to run"), and a screenshot then
	// waits for a paint that never arrives. Reloading gets a renderer that works.
	await appWindow.reload();
	await appWindow.getByRole("main").waitFor();
	await expect(appWindow.getByRole("button", { name: /select project/i })).toBeVisible();
};

// Under a bare X server the renderer only paints a capturable frame on load: a
// screenshot taken after an in-app state change waits for a frame that never
// arrives. So every surface is reached by navigating or reloading, never by
// clicking, and is photographed immediately afterwards — which is also why each
// surface gets its own test rather than sharing a window.
const goToTab = async (
	appWindow: Page,
	tab: "workspace" | "upstream" | "branches",
): Promise<void> => {
	// The sidebar page lives in the query string, so this is a real navigation.
	await appWindow.evaluate((page) => {
		window.location.search = page === "workspace" ? "" : `?page=${page}`;
	}, tab);
	await appWindow.getByRole("main").waitFor();
	// Navigating alone leaves the renderer without a frame a screenshot can take;
	// only an explicit reload reliably produces one.
	await appWindow.reload();
	await appWindow.getByRole("main").waitFor();
	await expect(appWindow.getByRole("button", { name: /select project/i })).toBeVisible();

	// Assert the tab actually changed. Every other post-condition here is present
	// on all three tabs, so without this a navigation that silently stayed put
	// would write the workspace panel into branches-tab.png and pass.
	const expected = tab === "workspace" ? "" : `page=${tab}`;
	await expect.poll(async () => new URL(appWindow.url()).search).toContain(expected);
};

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
