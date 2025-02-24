import { clickByTestId, getByTestId, waitForTestId } from './util.ts';
import { expect, Page } from '@playwright/test';

export async function openBranchContextMenu(page: Page, branchName: string) {
	const branchHeader = getByTestId(page, 'branch-header').filter({
		hasText: branchName
	});
	await expect(branchHeader).toBeVisible();
	branchHeader.click({ button: 'right' });

	// The context menu should be visible
	await waitForTestId(page, 'branch-header-context-menu');
}

/**
 * Delete a branch from a stack by its name.
 */
export async function deleteBranch(page: Page, branchName: string) {
	// Open the branch context menu
	await openBranchContextMenu(page, branchName);

	// Click the delete branch option
	await clickByTestId(page, 'branch-header-context-menu-delete');

	// The confirmation modal should be visible
	await waitForTestId(page, 'branch-header-delete-modal');
	// Confirm the deletion
	await clickByTestId(page, 'branch-header-delete-modal-action-button');
}

export async function unapplyStack(page: Page, branchName: string) {
	// Open the branch context menu
	await openBranchContextMenu(page, branchName);

	// Click the unapply stack option
	await clickByTestId(page, 'branch-header-context-menu-unapply-branch');
}

/**
 * Create a new branch with the given name.
 */
export async function createNewBranch(page: Page, branchName: string) {
	await clickByTestId(page, 'chrome-header-create-branch-button');
	const modal = await waitForTestId(page, 'create-new-branch-modal');

	const input = modal.locator('#new-branch-name-input');
	await input.fill(branchName);
	await clickByTestId(page, 'confirm-submit');
}
