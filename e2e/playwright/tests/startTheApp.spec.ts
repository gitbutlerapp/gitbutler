import { GIT_CONFIG_GLOBAL } from "../src/env.ts";
import { writeToFile } from "../src/file.ts";
import { gotoOnboarding, startGitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	clickByTestId,
	commitRow,
	fillByTestId,
	getByTestId,
	mockPickDirectory,
	textEditorFillByTestId,
	waitForTestId,
} from "../src/util.ts";
import { expect, type Page } from "@playwright/test";

async function addProjectAndOpenWorkspace(page: Page, projectPath: string) {
	await gotoOnboarding(page);

	await mockPickDirectory(page, projectPath);
	await clickByTestId(page, "add-local-project");

	await waitForTestId(page, "project-setup-page");
	clickByTestId(page, "set-base-branch");
	await waitForTestId(page, "workspace-view");
}

async function writeCommitAndAssert(page: Page, filePath: string) {
	writeToFile(filePath, "This is supper important content");

	const files = getByTestId(page, "file-list-item");
	await expect(files).toHaveCount(1);
	await expect(files.first()).toHaveText("test-file.txt");

	await clickByTestId(page, "commit-to-new-branch-button");
	await waitForTestId(page, "new-commit-view");

	const title = "New commit message";
	const body = "This is the body of the commit message.";
	await fillByTestId(page, "commit-drawer-title-input", title);
	await textEditorFillByTestId(page, "commit-drawer-description-input", body);
	await clickByTestId(page, "commit-drawer-action-button");

	const rows = commitRow(page);
	await expect(rows).toHaveCount(1);
	await expect(rows.first()).toHaveText(title);
}

test("should start the application and be able to commit", async ({ page, gitbutler }) => {
	await gitbutler.runScript("setup-empty-project.sh");

	const projectPath = gitbutler.pathInWorkdir("local-clone/");
	await addProjectAndOpenWorkspace(page, projectPath);
	await writeCommitAndAssert(page, gitbutler.pathInWorkdir("local-clone/test-file.txt"));
});

// This test needs a per-test git config path derived from testInfo, so it
// bypasses the gitbutler fixture and manages the lifecycle directly.
test("no author setup - should start the application and be able to commit", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	const otherGitConfig = testInfo.outputPath("config/gitconfig");

	const gitbutler = await startGitButler(
		workdir,
		configdir,
		context,
		{
			GIT_CONFIG_GLOBAL: otherGitConfig,
		},
		{ onboardingComplete: true },
	);

	try {
		// Setup uses the normal git config so the repo has an author —
		// only the running server lacks one.
		await gitbutler.runScript("setup-empty-project.sh", undefined, {
			GIT_CONFIG_GLOBAL,
		});

		await addProjectAndOpenWorkspace(page, gitbutler.pathInWorkdir("local-clone/"));

		await waitForTestId(page, "global-modal-author-missing");
		await fillByTestId(page, "global-modal-author-missing-name-input", "Test User");
		await fillByTestId(page, "global-modal-author-missing-email-input", "test@example.com");
		await clickByTestId(page, "global-modal-author-missing-action-button");
		await expect(getByTestId(page, "global-modal-author-missing")).toBeHidden();

		await writeCommitAndAssert(page, gitbutler.pathInWorkdir("local-clone/test-file.txt"));
	} finally {
		await gitbutler.destroy();
	}
});
