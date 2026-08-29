import { expect } from "../../src/expect.ts";
import { openWorkspace } from "../../src/setup.ts";
import { clickByTestId, getByTestId, stack, waitForTestId } from "../../src/util.ts";
import { type Locator, type Page } from "@playwright/test";
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

export function branchHeader(page: Page, branchName: string): Locator {
	return page.locator(`[data-testid="branch-header"][data-testid-branch-header="${branchName}"]`);
}

export async function createDependentBranch(
	page: Page,
	branchName: string,
	stackBranch?: string,
): Promise<void> {
	const addButton = stackBranch
		? stack(page)
				.filter({ has: branchHeader(page, stackBranch) })
				.getByTestId("branch-header-add-dependent-branch-button")
		: getByTestId(page, "branch-header-add-dependent-branch-button");
	await addButton.click();
	const modal = await waitForTestId(page, "branch-header-add-dependent-branch-modal");
	await modal.locator("input").fill(branchName);
	await clickByTestId(page, "branch-header-add-dependent-branch-modal-action-button");
	await expect(modal).toBeHidden();
}
