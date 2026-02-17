import { updateCommitMessage, verifyCommitMessageEditor } from "../src/commit.ts";
import {
	assertFileContent,
	assertFileIsStaged,
	assertFileIsUncommitted,
	assertFileIsUnstaged,
} from "../src/file.ts";
import { getBaseURL, type GitButler, startGitButler } from "../src/setup.ts";
import { clickByTestId, dragAndDropByLocator, getByTestId, waitForTestId } from "../src/util.ts";
import { expect, test } from "@playwright/test";
import { writeFileSync } from "fs";

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL(),
});

test.afterEach(async () => {
	await gitbutler.destroy();
});

test("should be able to start a commit by dragging a file", async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	const fileName = "b_file";
	const filePath = gitbutler.pathInWorkdir("local-clone/" + fileName);
	const fileContent = "Hello! this is file b\n";
	const anotherFileName = "c_file";
	const anotherFilePath = gitbutler.pathInWorkdir("local-clone/" + anotherFileName);
	const anotherFileContent = "Hello! this is file c\n";

	await gitbutler.runScript("project-with-remote-branches.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	// There should be only one stack
	const stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);
	const stack = stacks.first();
	await expect(stack).toContainText("branch1");

	// The stack should have two commits
	const commits = getByTestId(page, "commit-row");
	await expect(commits).toHaveCount(2);

	// Push the changes to the remote branch
	// (it's basically a no-op, just makes sure that the same commits after rebasing are on the remote)
	await clickByTestId(page, "stack-push-button");

	// Add a two new files to the workdir
	writeFileSync(filePath, fileContent, { flag: "w" });
	writeFileSync(anotherFilePath, anotherFileContent, { flag: "w" });

	const fileLocator = page
		.getByTestId("uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({
			hasText: fileName,
		});
	await expect(fileLocator).toHaveCount(1);
	await expect(fileLocator).toBeVisible();

	const branchCardLocators = await waitForTestId(page, "branch-card");
	const branchCardLocator = branchCardLocators.filter({
		hasText: "branch1",
	});

	// Drag the new file onto the top commit, to amend it
	await dragAndDropByLocator(page, fileLocator, branchCardLocator);

	// Verify that the only the dragged file is now staged
	await assertFileIsStaged(page, fileName);
	await assertFileIsUnstaged(page, anotherFileName);

	const commitTitle = "New file added";
	const commitMessage = `Added ${fileName} to the project`;

	// Fill the commit message
	await verifyCommitMessageEditor(page, "", "");
	await updateCommitMessage(page, commitTitle, commitMessage);

	// Submit the commit
	await clickByTestId(page, "commit-drawer-action-button");
	const commitRow = getByTestId(page, "commit-row").filter({ hasText: commitTitle });
	await expect(commitRow).toHaveCount(1);

	// Verify that the other file is still uncommitted
	await assertFileIsUncommitted(page, anotherFileName);

	// Verify the file contents are correct
	await assertFileContent(filePath, fileContent);
	await assertFileContent(anotherFilePath, anotherFileContent);
});
