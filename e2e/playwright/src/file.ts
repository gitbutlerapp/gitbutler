import { clickByTestId, getByTestId, waitForTestId } from './util.ts';
import { expect, Page } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';

/**
 * Write text to a file.
 *
 * The file and directory will be created if they do not exist.
 */
export function writeToFile(filePath: string, content: string): void {
	ensureDirectoryExists(filePath);
	fs.writeFileSync(filePath, content, { flag: 'w+', encoding: 'utf-8' });
}

function ensureDirectoryExists(filePath: string): void {
	const dir = path.dirname(filePath);
	if (!fs.existsSync(dir)) {
		fs.mkdirSync(dir, { recursive: true });
	}
}

/**
 * Write multiple files.
 */
export function writeFiles(files: Record<string, string>): void {
	for (const [filePath, content] of Object.entries(files)) {
		writeToFile(filePath, content);
	}
}

export async function assertFileContent(filePath: string, expectedContent: string): Promise<void> {
	const actualContent = fs.readFileSync(filePath, 'utf-8');
	await expect.poll(() => actualContent).toBe(expectedContent);
}

/**
 * Discard a file from the uncommitted changes list via context menu.
 */
export async function discardFile(fileName: string, page: Page): Promise<void> {
	let fileItem = getByTestId(page, 'uncommitted-changes-file-list')
		.getByTestId('file-list-item')
		.filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();

	await fileItem.click({ button: 'right' });
	await clickByTestId(page, 'file-list-item-context-menu__discard-changes');

	await waitForTestId(page, 'discard-file-changes-confirmation-modal');
	await clickByTestId(page, 'discard-file-changes-confirmation-modal-discard');

	fileItem = getByTestId(page, 'uncommitted-changes-file-list')
		.getByTestId('file-list-item')
		.filter({ hasText: fileName });
	await expect(fileItem).not.toBeVisible();
}
