import { expectCurrentBranchChip, openSingleBranchWorkspace } from "./helpers.ts";
import { assertBranch, assertCleanWorktree, assertCommitSubjects } from "../../src/branch.ts";
import { test } from "../../src/test.ts";
import {
	clickByTestId,
	commitRow,
	getByTestId,
	stack,
	waitForTestId,
	waitForTestIdToNotExist,
} from "../../src/util.ts";
import { expect, type Page } from "@playwright/test";
import { execFileSync } from "node:child_process";

const FULLY_INTEGRATED_BRANCH = "fully-integrated-branch";
const PARTIAL_STACK_BASE = "partial-stack-base";
const PARTIAL_STACK_TOP = "partial-stack-top";
const REBASED_SINGLE_BRANCH = "rebased-single-branch";
const TARGET_BRANCH = "master";
const TARGET_REMOTE_BRANCH = "origin/master";
const ERROR_TOAST_PATTERN = /API error:|error/i;

test.use({
	gitbutlerOptions: {
		config: {
			onboardingComplete: true,
			featureFlags: { singleBranch: true },
		},
	},
});

async function syncAndIntegrateWorkspace(page: Page) {
	await clickByTestId(page, "sync-button");
	await expectNoErrorToast(page);
	await clickByTestId(page, "integrate-upstream-commits-button");
	await expectNoErrorToast(page);
	await clickByTestId(page, "integrate-upstream-action-button");
	await expectNoErrorToast(page);
}

async function expectNoErrorToast(page: Page) {
	await expect(
		page.getByTestId("toast-info-message").filter({ hasText: ERROR_TOAST_PATTERN }),
	).toHaveCount(0);
}

async function expectBranchParentToBeOriginMaster(pathToRepo: string, branchName: string) {
	await expect
		.poll(() => git(pathToRepo, ["rev-parse", `${branchName}~2`]), {
			message: `Expected ${branchName} to be rebased onto origin/master`,
			intervals: [100, 200, 500, 1000],
		})
		.toBe(git(pathToRepo, ["rev-parse", "origin/master"]));
}

async function expectBranchFirstParentToBeOriginMaster(pathToRepo: string, branchName: string) {
	await expect
		.poll(() => git(pathToRepo, ["rev-parse", `${branchName}^`]), {
			message: `Expected ${branchName} to be parented to origin/master`,
			intervals: [100, 200, 500, 1000],
		})
		.toBe(git(pathToRepo, ["rev-parse", "origin/master"]));
}

async function expectLocalBranchNotToExist(pathToRepo: string, branchName: string) {
	await expect
		.poll(() => gitSucceeds(pathToRepo, ["show-ref", "--verify", `refs/heads/${branchName}`]), {
			message: `Expected local branch ${branchName} to be deleted`,
			intervals: [100, 200, 500, 1000],
		})
		.toBe(false);
}

async function replacementBranchAtTarget(pathToRepo: string): Promise<string> {
	await expect
		.poll(() => findReplacementBranchAtTarget(pathToRepo), {
			message: `Expected a new local branch at ${TARGET_REMOTE_BRANCH}`,
			intervals: [100, 200, 500, 1000],
		})
		.not.toBeUndefined();
	const replacementBranch = findReplacementBranchAtTarget(pathToRepo);
	expect(replacementBranch).toBeDefined();
	return replacementBranch!;
}

function findReplacementBranchAtTarget(pathToRepo: string): string | undefined {
	const targetCommit = git(pathToRepo, ["rev-parse", TARGET_REMOTE_BRANCH]);
	return git(pathToRepo, ["for-each-ref", "--format=%(refname:short) %(objectname)", "refs/heads"])
		.split("\n")
		.map((line) => line.trim().split(" "))
		.find(([branchName, commitId]) => {
			return (
				!branchName.startsWith("gitbutler/") &&
				branchName !== TARGET_BRANCH &&
				branchName !== FULLY_INTEGRATED_BRANCH &&
				commitId === targetCommit
			);
		})?.[0];
}

function git(pathToRepo: string, args: string[]): string {
	return execFileSync("git", args, {
		cwd: pathToRepo,
		encoding: "utf8",
	}).trim();
}

function gitSucceeds(pathToRepo: string, args: string[]): boolean {
	try {
		git(pathToRepo, args);
		return true;
	} catch {
		return false;
	}
}

test("creates a new branch on top of the advanced target after branch is fully integrated", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-in-single-branch-upstream-integration.sh", [
		"fully-integrated",
	]);
	await openSingleBranchWorkspace(page);

	const localClone = gitbutler.pathInWorkdir("local-clone");
	await assertBranch(FULLY_INTEGRATED_BRANCH, localClone);
	await expectCurrentBranchChip(page, FULLY_INTEGRATED_BRANCH);
	await expect(commitRow(page, "fully-integrated: second commit")).toBeVisible();

	await gitbutler.runScript("merge-upstream-branch-to-base.sh", [FULLY_INTEGRATED_BRANCH]);
	await syncAndIntegrateWorkspace(page);

	await expectLocalBranchNotToExist(localClone, FULLY_INTEGRATED_BRANCH);
	const replacementBranch = await replacementBranchAtTarget(localClone);
	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-card")).toContainText(replacementBranch);
	await assertBranch(replacementBranch, localClone);
	await expectCurrentBranchChip(page, replacementBranch);
	expect(git(localClone, ["rev-parse", replacementBranch])).toBe(
		git(localClone, ["rev-parse", TARGET_REMOTE_BRANCH]),
	);
	await assertCleanWorktree(localClone);
	await expectNoErrorToast(page);
});

test("keeps the top branch when its lower stack segment is integrated", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-in-single-branch-upstream-integration.sh", ["partial-stack"]);
	await openSingleBranchWorkspace(page);

	const localClone = gitbutler.pathInWorkdir("local-clone");
	await assertBranch(PARTIAL_STACK_TOP, localClone);
	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-card")).toHaveCount(2);
	await expect(
		getByTestId(page, "branch-card").filter({ hasText: PARTIAL_STACK_BASE }),
	).toBeVisible();
	await expect(
		getByTestId(page, "branch-card").filter({ hasText: PARTIAL_STACK_TOP }),
	).toBeVisible();

	await gitbutler.runScript("merge-upstream-branch-to-base.sh", [PARTIAL_STACK_BASE]);
	await clickByTestId(page, "sync-button");
	await expectNoErrorToast(page);
	await clickByTestId(page, "integrate-upstream-commits-button");
	await expectNoErrorToast(page);

	const baseRow = page
		.locator(`[data-integration-row-branch-name="${PARTIAL_STACK_BASE}"]`)
		.first();
	await expect(baseRow.getByTestId("integrate-upstream-series-row-status-badge")).toHaveText(
		"Integrated",
	);

	await clickByTestId(page, "integrate-upstream-action-button");
	await expectNoErrorToast(page);

	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-card")).toHaveCount(1);
	await expect(getByTestId(page, "branch-card")).toContainText(PARTIAL_STACK_TOP);
	await expect(getByTestId(page, "branch-card")).not.toContainText(PARTIAL_STACK_BASE);
	await assertBranch(PARTIAL_STACK_TOP, localClone);
	await expectCurrentBranchChip(page, PARTIAL_STACK_TOP);
	await assertCommitSubjects(
		["partial-stack-top: first commit", `Merging upstream branch ${PARTIAL_STACK_BASE} into base`],
		localClone,
	);
	await expectBranchFirstParentToBeOriginMaster(localClone, PARTIAL_STACK_TOP);
	await assertCleanWorktree(localClone);
	await expectNoErrorToast(page);
});

test("rebases the checked-out branch when the target advances", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-in-single-branch-upstream-integration.sh", ["rebase"]);
	await openSingleBranchWorkspace(page);

	const localClone = gitbutler.pathInWorkdir("local-clone");
	await assertBranch(REBASED_SINGLE_BRANCH, localClone);
	await expectCurrentBranchChip(page, REBASED_SINGLE_BRANCH);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-base.sh");
	await syncAndIntegrateWorkspace(page);

	await waitForTestIdToNotExist(page, "integrate-upstream-commits-button");
	await assertBranch(REBASED_SINGLE_BRANCH, localClone);
	await expectCurrentBranchChip(page, REBASED_SINGLE_BRANCH);
	await assertCommitSubjects(
		[
			"rebased-single-branch: second commit",
			"rebased-single-branch: first commit",
			"commit in base",
		],
		localClone,
	);
	await expectBranchParentToBeOriginMaster(localClone, REBASED_SINGLE_BRANCH);
	await assertCleanWorktree(localClone);
	await expectNoErrorToast(page);
});

test("shows uncommitted worktree conflicts when target advances with no stacks", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openSingleBranchWorkspace(page);

	await expect(stack(page)).toHaveCount(0);

	await gitbutler.runScript(
		"project-with-remote-branches__add-conflicting-base-and-dirty-worktree.sh",
	);
	await clickByTestId(page, "sync-button");
	await expectNoErrorToast(page);
	await clickByTestId(page, "integrate-upstream-commits-button");
	await expectNoErrorToast(page);

	const worktreeConflicts = await waitForTestId(page, "integrate-upstream-worktree-conflicts");
	await expect(worktreeConflicts).toContainText("a_file");
});
