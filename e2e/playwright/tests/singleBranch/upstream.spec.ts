import {
	expectCurrentBranchChip,
	pushCurrentBranch,
	SINGLE_BRANCH_REMOTE_BRANCH,
	setupSingleBranchProjectWithRemoteBranch,
	SINGLE_BRANCH_NAME,
} from "./helpers.ts";
import {
	assertBranch,
	assertCleanWorktree,
	assertCommitSubjects,
	assertDirtyWorktree,
	assertRefNotEquals,
	assertStatusContains,
} from "../../src/branch.ts";
import { updateCommitMessage, verifyCommitMessageEditor } from "../../src/commit.ts";
import { writeToFile } from "../../src/file.ts";
import { test } from "../../src/test.ts";
import { fetchRemoteChanges } from "../../src/upstream.ts";
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

test("can push the checked-out branch to origin", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProjectWithRemoteBranch(gitbutler, page);

	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await pushCurrentBranch(page, localClone);
	await assertCleanWorktree(localClone);
	await expectCurrentBranchChip(page, SINGLE_BRANCH_NAME);
});

test("can sync a remote advancement for the checked-out branch", async ({ page, gitbutler }) => {
	const localClone = await setupSingleBranchProjectWithRemoteBranch(gitbutler, page);

	await pushCurrentBranch(page, localClone);
	await gitbutler.runScript("single-branch__add-remote-commit.sh");

	await fetchRemoteChanges(page);

	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertRefNotEquals("HEAD", SINGLE_BRANCH_REMOTE_BRANCH, localClone);
	await assertStatusContains("[behind 1]", localClone);
	await assertCleanWorktree(localClone);
	await assertCommitSubjects(
		["single-branch: add file", "single-branch: second commit", "single-branch: first commit"],
		localClone,
	);
	await expectCurrentBranchChip(page, SINGLE_BRANCH_NAME);
});

test("keeps single-branch mode when syncing a conflicting remote advancement", async ({
	page,
	gitbutler,
}) => {
	const localClone = await setupSingleBranchProjectWithRemoteBranch(gitbutler, page);
	const filePath = gitbutler.pathInWorkdir("local-clone", "single_branch_file.txt");

	await pushCurrentBranch(page, localClone);

	writeToFile(filePath, "local conflict content\n");
	await assertDirtyWorktree(localClone);

	await clickByTestId(page, "start-commit-button");
	await verifyCommitMessageEditor(page, "", "");
	await updateCommitMessage(page, "single-branch: local conflicting commit", "");
	await clickByTestId(page, "commit-drawer-action-button");

	await assertCleanWorktree(localClone);
	await gitbutler.runScript("single-branch__add-conflicting-remote-commit.sh");

	await fetchRemoteChanges(page);

	await expectCurrentBranchChip(page, SINGLE_BRANCH_NAME);
	await assertBranch(SINGLE_BRANCH_NAME, localClone);
	await assertRefNotEquals("HEAD", SINGLE_BRANCH_REMOTE_BRANCH, localClone);
	await assertStatusContains("[ahead 1, behind 1]", localClone);
	await expect(commitRow(page, "single-branch: local conflicting commit")).toBeVisible();
	await expect(getByTestId(page, "workspace-view")).toBeVisible();
});
