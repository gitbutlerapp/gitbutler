import { setupSingleBranchProject, SINGLE_BRANCH_NAME } from "./helpers.ts";
import { assertBranch, assertCleanWorktree, assertCommitSubjects } from "../../src/branch.ts";
import { test } from "../../src/test.ts";
import { commitRow, dragAndDropByLocator, getByTestId } from "../../src/util.ts";
import { expect } from "@playwright/test";

test.use({
	gitbutlerOptions: {
		config: {
			onboardingComplete: true,
			featureFlags: { singleBranch: true },
		},
	},
});

test("can squash commits by dragging one commit onto another", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);
	await assertCleanWorktree(localClone);

	await expect(commitRow(page)).toHaveCount(3);

	await dragAndDropByLocator(
		page,
		commitRow(page, "single-branch: add file"),
		commitRow(page, "single-branch: second commit"),
	);

	await expect(getByTestId(page, "global-modal-commit-failed")).toHaveCount(0);
	await expect(commitRow(page, "single-branch: first commit")).toBeVisible();
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertCommitSubjects(
		["single-branch: second commit", "single-branch: first commit"],
		localClone,
	);
});
