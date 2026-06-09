import {
	EMPTY_SINGLE_BRANCH_NAME,
	expectCurrentBranchChip,
	setupEmptySingleBranchProject,
	setupSingleBranchProjectWithRemoteBranch,
	SINGLE_BRANCH_NAME,
	TARGET_BRANCH,
} from "./helpers.ts";
import {
	assertBranch,
	assertCleanWorktree,
	assertDirtyWorktree,
	assertRefEquals,
} from "../../src/branch.ts";
import { updateCommitMessage, verifyCommitMessageEditor } from "../../src/commit.ts";
import { writeToFile } from "../../src/file.ts";
import { test } from "../../src/test.ts";
import { fetchRemoteChanges, integrateUpstreamChanges } from "../../src/upstream.ts";
import { clickByTestId, commitRow, getByTestId } from "../../src/util.ts";
import { expect } from "@playwright/test";

test.use({
	gitbutlerOptions: {
		config: {
			onboardingComplete: true,
			featureFlags: { singleBranch: true },
		},
	},
});

test.skip("shows target conflicts with dirty worktree changes", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProjectWithRemoteBranch(gitbutler, page);

	await gitbutler.runScript(
		"project-with-remote-branches__add-conflicting-base-and-dirty-worktree.sh",
	);
	await fetchRemoteChanges(page);
	await clickByTestId(page, "integrate-upstream-commits-button");

	const worktreeConflicts = getByTestId(page, "integrate-upstream-worktree-conflicts");
	await expect(worktreeConflicts).toContainText("a_file");
	await expectCurrentBranchChip(page, SINGLE_BRANCH_NAME);
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertDirtyWorktree(localClone);
});

test.skip("keeps single-branch mode when target advancement conflicts with a local commit", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProjectWithRemoteBranch(gitbutler, page);
	const filePath = gitbutler.pathInWorkdir("local-clone", "a_file");

	writeToFile(filePath, "local target conflict content\n");
	await assertDirtyWorktree(localClone);

	await clickByTestId(page, "start-commit-button");
	await verifyCommitMessageEditor(page, "", "");
	await updateCommitMessage(page, "single-branch: local target conflict", "");
	await clickByTestId(page, "commit-drawer-action-button");
	await assertCleanWorktree(localClone);

	await gitbutler.runScript("single-branch__add-conflicting-base-commit.sh");
	await integrateUpstreamChanges(page);

	await expectCurrentBranchChip(page, SINGLE_BRANCH_NAME);
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await expect(commitRow(page, "single-branch: local target conflict")).toBeVisible();
	await expect(getByTestId(page, "workspace-view")).toBeVisible();
});

test.skip("keeps an empty checked-out branch when the target advances", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupEmptySingleBranchProject(gitbutler, page);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-base.sh");
	await integrateUpstreamChanges(page);

	await expectCurrentBranchChip(page, EMPTY_SINGLE_BRANCH_NAME);
	await assertBranch(EMPTY_SINGLE_BRANCH_NAME, localClone);
	await assertRefEquals("HEAD", TARGET_BRANCH, localClone);
	await assertCleanWorktree(localClone);
	await expect(getByTestId(page, "workspace-view")).toBeVisible();
});
