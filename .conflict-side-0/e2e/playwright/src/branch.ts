import { clickByTestId, getByTestId, waitForTestId } from "./util.ts";
import { expect, Page } from "@playwright/test";
import { execFileSync } from "child_process";

export async function openBranchContextMenu(page: Page, branchName: string) {
	const branchHeader = getByTestId(page, "branch-header").filter({
		hasText: branchName,
	});
	await expect(branchHeader).toBeVisible();
	branchHeader.click({ button: "right" });

	// The context menu should be visible
	await waitForTestId(page, "branch-header-context-menu");
}

/**
 * Delete a branch from a stack by its name.
 */
export async function deleteBranch(page: Page, branchName: string) {
	// Open the branch context menu
	await openBranchContextMenu(page, branchName);

	// Click the delete branch option
	await clickByTestId(page, "branch-header-context-menu-delete");

	// The confirmation modal should be visible
	await waitForTestId(page, "branch-header-delete-modal");
	// Confirm the deletion
	await clickByTestId(page, "branch-header-delete-modal-action-button");
}

export async function unapplyStack(page: Page, branchName: string) {
	// Open the branch context menu
	await openBranchContextMenu(page, branchName);

	// Click the unapply stack option
	await clickByTestId(page, "branch-header-context-menu-unapply-branch");
}

/**
 * Create a new branch with the given name.
 */
export async function createNewBranch(page: Page, branchName: string) {
	await clickByTestId(page, "chrome-header-create-branch-button");
	const modal = await waitForTestId(page, "create-new-branch-modal");

	const input = modal.locator("#new-branch-name-input");
	await input.fill(branchName);
	await clickByTestId(page, "confirm-submit");
}

export async function assertBranch(branchName: string, pathToRepo: string): Promise<void> {
	await expect
		.poll(() => git(pathToRepo, ["branch", "--show-current"]), {
			message: `Expected branch name to be "${branchName}"`,
			intervals: [100, 200, 500, 1000],
		})
		.toBe(branchName);
}

export async function assertSymbolicHead(refName: string, pathToRepo: string): Promise<void> {
	await expect
		.poll(() => git(pathToRepo, ["symbolic-ref", "--short", "HEAD"]), {
			message: `Expected HEAD to point to "${refName}"`,
			intervals: [100, 200, 500, 1000],
		})
		.toBe(refName);
}

export async function assertCommitSubjects(
	expectedSubjects: string[],
	pathToRepo: string,
): Promise<void> {
	await expect
		.poll(() => commitSubjects(pathToRepo, expectedSubjects.length), {
			message: `Expected commit subjects to be ${JSON.stringify(expectedSubjects)}`,
			intervals: [100, 200, 500, 1000],
		})
		.toEqual(expectedSubjects);
}

export async function assertCleanWorktree(pathToRepo: string): Promise<void> {
	await expect
		.poll(() => git(pathToRepo, ["status", "--porcelain"]), {
			message: "Expected worktree to be clean",
			intervals: [100, 200, 500, 1000],
		})
		.toBe("");
}

export async function assertDirtyWorktree(pathToRepo: string): Promise<void> {
	await expect
		.poll(() => git(pathToRepo, ["status", "--porcelain"]), {
			message: "Expected worktree to be dirty",
			intervals: [100, 200, 500, 1000],
		})
		.not.toBe("");
}

export function currentBranch(pathToRepo: string): string {
	return git(pathToRepo, ["branch", "--show-current"]);
}

export function commitSubjects(pathToRepo: string, count: number): string[] {
	return git(pathToRepo, ["log", `--max-count=${count}`, "--format=%s"])
		.split("\n")
		.filter(Boolean);
}

function git(pathToRepo: string, args: string[]): string {
	return execFileSync("git", args, {
		cwd: pathToRepo,
		encoding: "utf8",
	}).trim();
}
