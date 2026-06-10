import { setupSingleBranchProject, SINGLE_BRANCH_NAME } from "./helpers.ts";
import {
	assertBranch,
	assertCleanWorktree,
	assertCommitSubjects,
	assertDirtyWorktree,
} from "../../src/branch.ts";
import {
	openCommitDrawer,
	startEditingCommitMessage,
	updateCommitMessage,
	verifyCommitDrawerContent,
	verifyCommitMessageEditor,
	verifyCommitPlaceholderPosition,
} from "../../src/commit.ts";
import { assertFileContent, unstageAllFiles, writeToFile } from "../../src/file.ts";
import { test } from "../../src/test.ts";
import { clickByTestId, commitRow, dragAndDropByLocator, getByTestId } from "../../src/util.ts";
import { expect } from "@playwright/test";

test.use({
	gitbutlerOptions: {
		config: {
			onboardingComplete: true,
			featureFlags: { singleBranch: true },
		},
	},
});

test("can commit new changes on the checked-out branch", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	await assertBranch(SINGLE_BRANCH_NAME, localClone);

	const fileName = "single_branch_new_file.txt";
	const fileContent = "new single branch content\n";
	writeToFile(gitbutler.pathInWorkdir("local-clone", fileName), fileContent);

	await expect(getByTestId(page, "file-list-item").filter({ hasText: fileName })).toBeVisible();
	await clickByTestId(page, "start-commit-button");
	await verifyCommitPlaceholderPosition(page);
	await unstageAllFiles(page);
	await getByTestId(page, "file-list-item")
		.filter({ hasText: fileName })
		.locator('input[type="checkbox"]')
		.click();

	const title = "single-branch: commit from e2e";
	const body = "Committed while HEAD is on a normal Git branch.";
	await verifyCommitMessageEditor(page, "", "");
	await updateCommitMessage(page, title, body);
	await clickByTestId(page, "commit-drawer-action-button");

	await expect(commitRow(page, title)).toBeVisible();
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertCleanWorktree(localClone);
	await assertFileContent(gitbutler.pathInWorkdir("local-clone", fileName), fileContent);
	await assertCommitSubjects(
		[
			title,
			"single-branch: add file",
			"single-branch: second commit",
			"single-branch: first commit",
		],
		localClone,
	);
});

test("can edit a commit message on the checked-out branch", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	const originalTitle = "single-branch: add file";
	const newTitle = "single-branch: reworded add file";
	const newBody = "Reworded in single-branch mode.";

	const drawer = await openCommitDrawer(page, originalTitle);
	await startEditingCommitMessage(page, drawer);
	await verifyCommitMessageEditor(page, originalTitle, "");

	await updateCommitMessage(page, newTitle, newBody);
	await clickByTestId(page, "commit-drawer-action-button");

	await verifyCommitDrawerContent(drawer, newTitle, newBody);
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertCleanWorktree(localClone);
	await assertCommitSubjects(
		[newTitle, "single-branch: second commit", "single-branch: first commit"],
		localClone,
	);
});

test("can amend file changes into an existing commit", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	const filePath = gitbutler.pathInWorkdir("local-clone", "single_branch_file.txt");
	const amendedContent = "single branch file\namended while in single-branch mode\n";
	writeToFile(filePath, amendedContent);
	await assertDirtyWorktree(localClone);

	const fileLocator = getByTestId(page, "file-list-item").filter({
		hasText: "single_branch_file.txt",
	});
	await expect(fileLocator).toBeVisible();

	await dragAndDropByLocator(page, fileLocator, commitRow(page, "single-branch: add file"));

	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertCleanWorktree(localClone);
	await assertFileContent(filePath, amendedContent);
	await assertCommitSubjects(
		["single-branch: add file", "single-branch: second commit", "single-branch: first commit"],
		localClone,
	);
});
