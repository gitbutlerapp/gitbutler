import { clickByTestId, getByTestId, waitForTestId } from "./util.ts";
import { expect, Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

/**
 * Write text to a file.
 *
 * The file and directory will be created if they do not exist.
 */
export function writeToFile(filePath: string, content: string): void {
	ensureDirectoryExists(filePath);
	fs.writeFileSync(filePath, content, { flag: "w+", encoding: "utf-8" });
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
	const actualContent = fs.readFileSync(filePath, "utf-8");
	await expect.poll(() => actualContent).toBe(expectedContent);
}

/**
 * Discard a file from the uncommitted changes list via context menu.
 */
export async function discardFile(fileName: string, page: Page): Promise<void> {
	let fileItem = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();

	await fileItem.click({ button: "right" });
	await clickByTestId(page, "file-list-item-context-menu__discard-changes");

	await waitForTestId(page, "discard-file-changes-confirmation-modal");
	await clickByTestId(page, "discard-file-changes-confirmation-modal-discard");

	fileItem = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await expect(fileItem).not.toBeVisible();
}

/**
 * Verify that there are no uncommitted changes.
 */
export async function assertNoUncommittedChanges(page: Page): Promise<void> {
	const uncommittedChangesList = getByTestId(page, "uncommitted-changes-file-list");
	await expect(uncommittedChangesList).toBeVisible();
	const fileItems = uncommittedChangesList.getByTestId("file-list-item");
	await expect(fileItems).toHaveCount(0);
}

/**
 * Stage the first file in the uncommitted changes list.
 */
export async function stageFirstFile(page: Page) {
	const fileItemCheckbox = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.first()
		.locator('input[type="checkbox"]');
	await expect(fileItemCheckbox).toBeVisible();
	await fileItemCheckbox.click();
}

/**
 * Verify that a file is staged for a commit.
 */
export async function assertFileIsStaged(page: Page, fileName: string) {
	const fileItemCheckbox = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName })
		.locator('input[type="checkbox"]');
	await expect(fileItemCheckbox).toBeVisible();
	await expect(fileItemCheckbox).toBeChecked();
}

/**
 * Verify that a file is unstaged for a commit.
 */
export async function assertFileIsUnstaged(page: Page, fileName: string) {
	const fileItemCheckbox = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName })
		.locator('input[type="checkbox"]');
	await expect(fileItemCheckbox).toBeVisible();
	await expect(fileItemCheckbox).not.toBeChecked();
}

/**
 * Unstage all files in the uncommitted changes list.
 */
export async function unstageAllFiles(page: Page) {
	const uncommittedFilesCheckbox = page
		.getByTestId("uncommitted-changes-header")
		.locator('input[type="checkbox"]');
	await expect(uncommittedFilesCheckbox).toBeVisible();
	await expect(uncommittedFilesCheckbox).toBeChecked();
	await uncommittedFilesCheckbox.click();
}

/**
 * Verify that a file is present in the uncommitted changes list.
 */
export async function assertFileIsUncommitted(page: Page, fileName: string) {
	const fileItem = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();
}
