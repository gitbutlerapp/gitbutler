import { assertBranch } from "../src/branch.ts";
import { getBaseURL, type GitButler, startGitButler } from "../src/setup.ts";
import { waitForTestId, waitForTestIdToNotExist } from "../src/util.ts";
import { test } from "@playwright/test";

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL(),
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

test("can switch back to workspace", async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript("project-with-remote-branches.sh");

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	await assertBranch("gitbutler/workspace", workdir + "/local-clone");

	// Switch to master branch
	await gitbutler.runScript("project-with-remote-branches__checkout-master.sh", ["local-clone"]);

	assertBranch("master", workdir + "/local-clone");

	// Button to switch back to workspace should be visible
	const switchButton = await waitForTestId(page, "chrome-header-switch-back-to-workspace-button");
	await switchButton.click();

	// Should be back on workspace
	await assertBranch("gitbutler/workspace", workdir + "/local-clone");

	// Button should no longer be visible
	await waitForTestIdToNotExist(page, "chrome-header-switch-back-to-workspace-button");
});
