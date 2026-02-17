import { assertFileContent, writeToFile } from "../src/file.ts";
import { getBaseURL, startGitButler, type GitButler } from "../src/setup.ts";
import { clickByTestId, waitForTestId } from "../src/util.ts";
import { expect, test } from "@playwright/test";

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL(),
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

test("should be able to absorb a file to a commit - simple", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	const filePath = gitbutler.pathInWorkdir("local-clone/a_file");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	const newFileContent = `This is the NEW content of the file.\n`;
	// Create a locked change by replacing the file content completely
	writeToFile(filePath, newFileContent);

	// Right-click the file to open the context menu
	const fileRow = (await waitForTestId(page, "file-list-item")).filter({ hasText: "a_file" });
	await fileRow.click({ button: "right" });

	// Click "Absorb changes" in the context menu
	await clickByTestId(page, "file-list-item-context-menu__absorb");

	// Wait for the absorb modal to appear
	await waitForTestId(page, "absorb-modal");

	const commitAbsorptionItems = await waitForTestId(page, "absorb-modal-commit-absorption");
	await expect(commitAbsorptionItems).toHaveCount(1);

	// Click the "Absorb changes" button in the modal
	await clickByTestId(page, "absorb-modal-action-button");

	// Verify that the file content has been absorbed
	await assertFileContent(filePath, newFileContent);

	// No more uncommitted changes should be present
	const files = await waitForTestId(page, "file-list-item");
	await expect(files).toHaveCount(0);
});
