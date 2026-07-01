import { openWorkspace } from "../../src/setup.ts";
import { expect, type Page } from "@playwright/test";
import type { GitButler } from "../../src/setup.ts";

export const SINGLE_BRANCH_NAME = "single-branch-fixture";

export async function setupSingleBranchProject(gitbutler: GitButler, page: Page): Promise<string> {
	await gitbutler.runScript("project-in-single-branch-mode.sh");
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
