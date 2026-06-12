import { openWorkspace } from "../../src/setup.ts";
import { assertRefEquals, assertRefExists } from "../../src/branch.ts";
import { pushFirstStack } from "../../src/upstream.ts";
import { expect, type Page } from "@playwright/test";
import type { GitButler } from "../../src/setup.ts";

export const SINGLE_BRANCH_NAME = "single-branch-fixture";
export const EMPTY_SINGLE_BRANCH_NAME = "single-branch-empty";
export const SINGLE_BRANCH_REMOTE_BRANCH = `refs/remotes/origin/${SINGLE_BRANCH_NAME}`;
export const TARGET_BRANCH = "refs/remotes/origin/master";

export async function setupSingleBranchProject(gitbutler: GitButler, page: Page): Promise<string> {
	await gitbutler.runScript("project-in-single-branch-mode.sh");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openSingleBranchWorkspace(page);
	return localClone;
}

export async function setupSingleBranchProjectWithRemoteBranch(
	gitbutler: GitButler,
	page: Page,
): Promise<string> {
	await gitbutler.runScript("project-in-single-branch-mode.sh");
	await gitbutler.runScript("single-branch__seed-remote-branch.sh");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openSingleBranchWorkspace(page);
	return localClone;
}

export async function setupEmptySingleBranchProject(
	gitbutler: GitButler,
	page: Page,
): Promise<string> {
	await gitbutler.runScript("project-in-empty-single-branch-mode.sh");
	const localClone = gitbutler.pathInWorkdir("local-clone");
	await openSingleBranchWorkspace(page);
	return localClone;
}

export async function openSingleBranchWorkspace(page: Page): Promise<void> {
	await openWorkspace(page);
	await expect(page.getByTestId("workspace-view")).toBeVisible();
}

export async function expectCurrentBranchChip(page: Page, branchName: string): Promise<void> {
	await expect(page.getByTestId("chrome-header-current-branch")).toContainText(branchName);
}

export async function pushCurrentBranch(page: Page, localClone: string): Promise<void> {
	await pushFirstStack(page);
	await assertRefExists(SINGLE_BRANCH_REMOTE_BRANCH, localClone);
	await assertRefEquals("HEAD", SINGLE_BRANCH_REMOTE_BRANCH, localClone);
}
