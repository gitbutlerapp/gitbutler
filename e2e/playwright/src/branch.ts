import { clickByTestId, getByTestId, waitForTestId } from './util.ts';
import { expect, Page } from '@playwright/test';

/**
 * Delete a branch from a stack by its name.
 */
export async function deleteBranch(page: Page, branchName: string) {
	const branchHeader = getByTestId(page, 'branch-header').filter({
		hasText: branchName || 'branch1'
	});
	await expect(branchHeader).toBeVisible();
	branchHeader.click({ button: 'right' });

	// The context menu should be visible
	await waitForTestId(page, 'branch-header-context-menu');

	// Click the delete branch option
	await clickByTestId(page, 'branch-header-context-menu-delete');

	// The confirmation modal should be visible
	await waitForTestId(page, 'branch-header-delete-modal');
	// Confirm the deletion
	await clickByTestId(page, 'branch-header-delete-modal-action-button');
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
