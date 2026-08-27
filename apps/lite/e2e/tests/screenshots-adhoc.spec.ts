// Ad-hoc captures for the branch-divergence work (GB-1496): the remote leg in
// the workspace sidebar and the update-from-remote flow in the details panel.
// No fixture reaches a diverged applied branch, so each test builds the state
// itself: apply branch1, rewrite its remote side, fetch.
//
// Kept out of the catalogue for now, while the surfaces are iterated on. Run
// it alone — never in the same command as screenshots.spec.ts, whose beforeAll
// wipes the output directory mid-run:
//
//   env -u ELECTRON_RUN_AS_NODE SCREENSHOT_OUT=adhoc-head BUT=$PWD/target/debug/but \
//     pnpm -F @gitbutler/lite test:e2e screenshots-adhoc

import type { Page } from "@playwright/test";
import { divergeBoth, divergeBranch1, rewriteBranch1Tip } from "../diverge.ts";
import { enabled, openProject, shoot } from "../screenshot-helpers.ts";
import type { LiteTestEnvironment } from "../setup.ts";
import { expect, test } from "../test.ts";

const diverge = async (appWindow: Page, environment: LiteTestEnvironment): Promise<void> => {
	divergeBranch1(environment);
	await appWindow.reload();
	await appWindow.getByRole("main").waitFor();
	await expect(appWindow.getByText("origin/branch1")).toBeVisible();
};

test.describe("screenshots", () => {
	test.skip(!enabled, "set SCREENSHOT_OUT to capture screenshots");
	// Each test launches Electron, seeds a repository, and rebuilds the
	// divergence before its one screenshot; the 30s default is not enough.
	test.describe.configure({ timeout: 180_000 });
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("diverged sidebar", async ({ appWindow, testEnvironment }) => {
		await openProject(appWindow);
		await diverge(appWindow, testEnvironment);
		await shoot(appWindow, "workspace-sidebar-diverged", "#sidebar-panel");
	});

	test("remote leg expanded", async ({ appWindow, testEnvironment }) => {
		await openProject(appWindow);
		await diverge(appWindow, testEnvironment);
		await appWindow.getByLabel(/^Expand origin\//).click();
		await expect(appWindow.getByText("Rework the parser entry point")).toBeVisible();
		await shoot(appWindow, "workspace-sidebar-remote-leg", "#sidebar-panel");
	});

	test("rewritten leg expanded", async ({ appWindow, testEnvironment }) => {
		await openProject(appWindow);
		rewriteBranch1Tip(testEnvironment);
		await appWindow.reload();
		await appWindow.getByRole("main").waitFor();
		await expect(appWindow.getByText("rewritten", { exact: true })).toBeVisible();
		await shoot(appWindow, "workspace-sidebar-rewritten-quiet", "#sidebar-panel");
		await appWindow.getByLabel("Expand origin/branch1").click();
		await expect(appWindow.getByText("branch1: second commit")).toBeVisible();
		await shoot(appWindow, "workspace-sidebar-rewritten-leg", "#sidebar-panel");
	});

	const dialog = '[aria-labelledby="branch-update-heading"]';

	test("update dialog", async ({ appWindow, testEnvironment }) => {
		await openProject(appWindow);
		await diverge(appWindow, testEnvironment);
		await appWindow.getByLabel("Integrate origin/branch1 into branch1").click();
		// The summary line renders once the dry-run preview is in.
		await expect(appWindow.getByText(/incoming commit/)).toBeVisible();
		await shoot(appWindow, "branch-update-dialog", dialog);
	});
	test("update dialog done state", async ({ appWindow, testEnvironment }) => {
		await openProject(appWindow);
		divergeBoth(testEnvironment);
		await appWindow.reload();
		await appWindow.getByRole("main").waitFor();
		await appWindow.getByLabel("Integrate origin/branch1 into branch1").click();
		await expect(appWindow.getByText(/incoming commit/)).toBeVisible();
		const integrate = appWindow.locator(dialog).getByRole("button", { name: "Integrate" });
		await expect(integrate).toBeEnabled();
		await integrate.click();
		// Integrating leaves the push to do, offered in place.
		await expect(appWindow.getByText("Branch updated.")).toBeVisible();
		await shoot(appWindow, "branch-update-done", dialog);
	});
});
