import { openWorkspace } from "../../src/setup.ts";
import { clickByTestId, getByTestId, waitForTestId } from "../../src/util.ts";
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

export async function applyBranchFromBranchesView(page: Page, branchName: string): Promise<void> {
	await clickByTestId(page, "navigation-branches-button");
	await waitForTestId(page, "branches-view");

	await getByTestId(page, "branches-view").getByText(branchName, { exact: true }).click();
	await waitForTestId(page, "branches-view-apply-branch-button");

	await clickByTestId(page, "branches-view-apply-branch-button");
	await waitForTestId(page, "workspace-view");
}
