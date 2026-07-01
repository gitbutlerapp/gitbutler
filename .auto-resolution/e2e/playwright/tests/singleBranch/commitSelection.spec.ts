import { setupSingleBranchProject, SINGLE_BRANCH_NAME } from "./helpers.ts";
import { assertBranch, assertCommitSubjects, assertDirtyWorktree } from "../../src/branch.ts";
import { assertFileIsUncommitted } from "../../src/file.ts";
import { test } from "../../src/test.ts";
import {
	commitRow,
	getByTestId,
	MOD_KEY,
	waitForElementToStabilize,
	waitForTestId,
} from "../../src/util.ts";
import { expect } from "@playwright/test";

test.use({
	gitbutlerOptions: {
		config: {
			onboardingComplete: true,
			featureFlags: { singleBranch: true },
		},
	},
});

test("can squash selected commits on the checked-out branch", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	const top = commitRow(page, "single-branch: add file");
	await top.click();
	await commitRow(page, "single-branch: second commit").click({ modifiers: [MOD_KEY] });
	await top.click({ button: "right" });

	const squashItem = await waitForTestId(page, "commit-row-context-menu-squash-selected");
	await waitForElementToStabilize(page, squashItem);
	await expect(squashItem).toContainText("Squash 2 commits");
	await squashItem.click();

	await expect(commitRow(page, "single-branch: first commit")).toBeVisible();
	await expect(getByTestId(page, "global-modal-commit-failed")).toHaveCount(0);
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertCommitSubjects(
		["single-branch: second commit", "single-branch: first commit"],
		localClone,
	);
});

test("can uncommit selected commits back into the worktree", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProject(gitbutler, page);

	const top = commitRow(page, "single-branch: add file");
	await top.click();
	await commitRow(page, "single-branch: second commit").click({ modifiers: [MOD_KEY] });
	await top.click({ button: "right" });

	const uncommitItem = await waitForTestId(page, "commit-row-context-menu-uncommit-selected");
	await waitForElementToStabilize(page, uncommitItem);
	await expect(uncommitItem).toContainText("Uncommit 2 commits");
	await uncommitItem.click();

	await expect(commitRow(page, "single-branch: first commit")).toBeVisible();
	await assertFileIsUncommitted(page, "single_branch_file.txt");
	await assertFileIsUncommitted(page, "single_branch_second.txt");
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertDirtyWorktree(localClone);
	await assertCommitSubjects(["single-branch: first commit", "base: initial commit"], localClone);
});
