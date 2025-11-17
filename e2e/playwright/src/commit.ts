import { clickByTestId, fillByTestId, getByTestId, textEditorFillByTestId } from './util.ts';
import { expect, Locator, Page } from '@playwright/test';

/**
 * Open a commit drawer by clicking on the commit row.
 */
export async function openCommitDrawer(page: Page, commitTitle: string) {
	const commitRow = getByTestId(page, 'commit-row').filter({ hasText: commitTitle });
	await expect(commitRow).toHaveCount(1);
	await expect(commitRow).toBeVisible();
	await commitRow.click();

	const commitDrawer = getByTestId(page, 'commit-drawer');
	await expect(commitDrawer).toBeVisible();
	return commitDrawer;
}

/**
 * Verify the commit drawer shows the expected title and description.
 */
export async function verifyCommitDrawerContent(
	commitDrawer: Locator,
	expectedTitle: string,
	expectedDescription: string
) {
	const commitDrawerTitle = commitDrawer.getByTestId('commit-drawer-title');
	await expect(commitDrawerTitle).toBeVisible();
	await expect(commitDrawerTitle).toContainText(expectedTitle);
	const commitDrawerDescription = commitDrawer.getByTestId('commit-drawer-description');
	await expect(commitDrawerDescription).toBeVisible();
	await expect(commitDrawerDescription).toContainText(expectedDescription);
}

/**
 * Open the kebab menu and start editing the commit message.
 */
export async function startEditingCommitMessage(page: Page, commitDrawer: Locator) {
	const commitKebabMenuButton = commitDrawer.getByTestId('kebab-menu-btn');
	await expect(commitKebabMenuButton).toBeVisible();
	await commitKebabMenuButton.click();
	await clickByTestId(page, 'commit-row-context-menu-edit-message-menu-btn');
}

/**
 * Verify the commit message editor contains the expected values.
 */
export async function verifyCommitMessageEditor(
	page: Page,
	expectedTitle: string,
	expectedMessage: string
) {
	const commitTitleInput = getByTestId(page, 'commit-drawer-title-input');
	await expect(commitTitleInput).toBeVisible();
	await expect(commitTitleInput).toHaveValue(expectedTitle);
	const commitBodyInput = getByTestId(page, 'commit-drawer-description-input');
	await expect(commitBodyInput).toBeVisible();
	await expect(commitBodyInput).toContainText(expectedMessage);
}

/**
 * Update the commit title and description in the editor.
 */
export async function updateCommitMessage(page: Page, newTitle: string, newMessage: string) {
	await fillByTestId(page, 'commit-drawer-title-input', newTitle);
	await textEditorFillByTestId(page, 'commit-drawer-description-input', newMessage);
}

/**
 * Verify the 'Your commit goes here' placeholder is visible and in the correct position.
 */
export async function verifyCommitPlaceholderPosition(page: Page) {
	const commitTargetPosition = getByTestId(page, 'your-commit-goes-here');
	await expect(commitTargetPosition).toBeVisible();
	await expect(commitTargetPosition).toHaveCount(1);
	await expect(commitTargetPosition).toContainClass('first');
}

/**
 * Unstage all files in the uncommitted changes list.
 */
export async function unstageAllFiles(page: Page) {
	const uncommittedFilesCheckbox = page
		.getByTestId('uncommitted-changes-header')
		.locator('input[type="checkbox"]');
	await expect(uncommittedFilesCheckbox).toBeVisible();
	await expect(uncommittedFilesCheckbox).toBeChecked();
	await uncommittedFilesCheckbox.click();
}

/**
 * Stage the first file in the uncommitted changes list.
 */
export async function stageFirstFile(page: Page) {
	const fileItemCheckbox = getByTestId(page, 'uncommitted-changes-file-list')
		.getByTestId('file-list-item')
		.first()
		.locator('input[type="checkbox"]');
	await expect(fileItemCheckbox).toBeVisible();
	await fileItemCheckbox.click();
}
