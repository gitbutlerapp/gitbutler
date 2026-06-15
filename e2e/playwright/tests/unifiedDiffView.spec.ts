import { discardFile } from "../src/file.ts";
import { getHunkHeaderSelector, getHunkLineSelector } from "../src/hunk.ts";
import { openWorkspace, type GitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	clickByTestId,
	commitRow,
	fillByTestId,
	getByTestId,
	waitForTestId,
	waitForTestIdToNotExist,
} from "../src/util.ts";
import { expect, type Locator, type Page } from "@playwright/test";
import { existsSync, readFileSync, writeFileSync } from "fs";
import { join, resolve } from "path";

const BIG_FILE_PATH_BEFORE = resolve(import.meta.dirname, "../fixtures/big-file_before.md");
const BIG_FILE_PATH_AFTER = resolve(import.meta.dirname, "../fixtures/big-file_after.md");
const BIG_FILE_PATH_AFTER_SMALL_CHANGES = resolve(
	import.meta.dirname,
	"../fixtures/big-file_after-small-changes.md",
);
const SINGLE_HUNK_BASE = [
	"one",
	"two",
	"three",
	"four",
	"five",
	"six",
	"seven",
	"eight",
	"nine",
	"ten",
].join("\n");
const SINGLE_HUNK_CHANGED = [
	"one",
	"two",
	"three",
	"four changed",
	"five",
	"six",
	"seven",
	"eight",
	"nine",
	"ten",
].join("\n");
const MULTI_HUNK_BASE = [
	"one",
	"two",
	"three",
	"four",
	"five",
	"six",
	"seven",
	"eight",
	"nine",
	"ten",
	"eleven",
	"twelve",
	"thirteen",
	"fourteen",
	"fifteen",
	"sixteen",
	"seventeen",
	"eighteen",
	"nineteen",
	"twenty",
].join("\n");
const MULTI_HUNK_CHANGED = [
	"one",
	"two changed",
	"three",
	"four",
	"five",
	"six",
	"seven",
	"eight",
	"nine",
	"ten",
	"eleven",
	"twelve",
	"thirteen",
	"fourteen",
	"fifteen",
	"sixteen",
	"seventeen",
	"eighteen changed",
	"nineteen",
	"twenty",
].join("\n");

/**
 * Seed a remote-base commit with the given file content, clone it into a fresh
 * local project, and delete the original local-clone so the test sees only the
 * new project. Returns the local path of the seeded file.
 */
async function seedRemoteFileAndCloneFresh(
	gitbutler: GitButler,
	projectName: string,
	fileName: string,
	contentBefore: string,
	commitMessage: string,
): Promise<string> {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await gitbutler.runScript("project-with-remote-branches__commit-file-into-remote-base.sh", [
		commitMessage,
		fileName,
		contentBefore,
	]);
	await gitbutler.runScript("project-with-remote-branches__clone-into-new-project.sh", [
		projectName,
	]);
	await gitbutler.runScript("project-with-remote-branches__delete-project.sh", ["local-clone"]);
	return join(gitbutler.pathInWorkdir(projectName + "/"), fileName);
}

async function startCommitWith(page: Page, fileName: string, title: string) {
	await clickByTestId(page, "start-commit-button");
	const fileItem = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();
	await fileItem.click();
	await expect(getByTestId(page, "unified-diff-view")).toBeVisible();
	await fillByTestId(page, "commit-drawer-title-input", title);
}

test("should be able to select the hunks correctly in a complex file", async ({
	page,
	gitbutler,
}) => {
	const fileName = "big-file.md";
	const bigFilePath = await seedRemoteFileAndCloneFresh(
		gitbutler,
		"my-new-project",
		fileName,
		readFileSync(BIG_FILE_PATH_BEFORE, "utf-8"),
		"Create big file with complex changes",
	);

	await openWorkspace(page);

	writeFileSync(bigFilePath, readFileSync(BIG_FILE_PATH_AFTER, "utf-8"), "utf-8");

	await clickByTestId(page, "commit-to-new-branch-button");

	const uncommittedFile = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await uncommittedFile.click();

	const unifiedDiffView = getByTestId(page, "unified-diff-view");
	await expect(unifiedDiffView).toBeVisible();

	// Part 1 — partial commit with some lines unselected
	await unselectHunkLines(fileName, unifiedDiffView, [1, 5, 9, 11, 13, 19, 23]);
	await fillByTestId(page, "commit-drawer-title-input", "Partial commit: Part 1");
	await clickByTestId(page, "commit-drawer-action-button");
	await waitForTestIdToNotExist(page, "new-commit-view");

	// Part 2 — another partial commit
	await startCommitWith(page, fileName, "Partial commit: Part 2");
	await unselectHunkLines(fileName, unifiedDiffView, [1, 5, 9, 11]);
	await clickByTestId(page, "commit-drawer-action-button");
	await waitForTestIdToNotExist(page, "new-commit-view");

	// Part 3 — full remainder
	await clickByTestId(page, "start-commit-button");
	await fillByTestId(page, "commit-drawer-title-input", "Full commit: Part 3");
	await clickByTestId(page, "commit-drawer-action-button");

	await expect(commitRow(page)).toHaveCount(3);
});

test("should unselect a complete hunk", async ({ page, gitbutler }) => {
	const fileName = "hunk-file.md";
	const filePath = await seedRemoteFileAndCloneFresh(
		gitbutler,
		"hunk-unselect-project",
		fileName,
		readFileSync(BIG_FILE_PATH_BEFORE, "utf-8"),
		"Initial file for hunk unselect",
	);

	await openWorkspace(page);

	writeFileSync(filePath, readFileSync(BIG_FILE_PATH_AFTER_SMALL_CHANGES, "utf-8"), "utf-8");

	await clickByTestId(page, "commit-to-new-branch-button");

	const fileItem = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await fileItem.click();

	const unifiedDiffView = getByTestId(page, "unified-diff-view");
	await expect(unifiedDiffView).toBeVisible();

	await unselectHunk(fileName, unifiedDiffView, 0);

	await fillByTestId(page, "commit-drawer-title-input", "Partial commit: Part 1");
	await clickByTestId(page, "commit-drawer-action-button");

	await discardFile(fileName, page);

	expect(readFileSync(filePath, "utf-8")).toMatchSnapshot();
});

test("should uncommit the only hunk in a commit back to unstaged changes", async ({
	page,
	gitbutler,
}) => {
	const fileName = "single-hunk-file.md";
	const commitTitle = "Single hunk commit";
	const filePath = await seedRemoteFileAndCloneFresh(
		gitbutler,
		"single-hunk-uncommit-project",
		fileName,
		SINGLE_HUNK_BASE,
		"Initial file for single hunk uncommit",
	);

	await openWorkspace(page);

	writeFileSync(filePath, `${SINGLE_HUNK_CHANGED}\n`, "utf-8");

	await commitFileToNewBranch(page, fileName, commitTitle);
	await uncommitCommittedHunk(page, fileName, commitTitle, 0);

	await expect(commitRow(page, commitTitle)).toBeVisible();
	await expect(committedFileItem(page, fileName, commitTitle)).toHaveCount(0);
	await expect(
		getByTestId(page, "uncommitted-changes-file-list")
			.getByTestId("file-list-item")
			.filter({ hasText: fileName }),
	).toBeVisible();
});

test("should uncommit one hunk and keep the remaining commit changes", async ({
	page,
	gitbutler,
}) => {
	const fileName = "multi-hunk-file.md";
	const commitTitle = "Multi hunk commit";
	const filePath = await seedRemoteFileAndCloneFresh(
		gitbutler,
		"multi-hunk-uncommit-project",
		fileName,
		MULTI_HUNK_BASE,
		"Initial file for multi hunk uncommit",
	);

	await openWorkspace(page);

	writeFileSync(filePath, `${MULTI_HUNK_CHANGED}\n`, "utf-8");

	await commitFileToNewBranch(page, fileName, commitTitle);
	await uncommitCommittedHunk(page, fileName, commitTitle, 0);

	await expect(commitRow(page, commitTitle)).toBeVisible();
	await expect(
		getByTestId(page, "uncommitted-changes-file-list")
			.getByTestId("file-list-item")
			.filter({ hasText: fileName }),
	).toBeVisible();

	const unifiedDiffView = await openCommittedFileDiff(page, fileName, commitTitle);
	await expect(unifiedDiffView.locator(getHunkHeaderSelector(fileName, 0))).toBeVisible();
	await expect(unifiedDiffView.locator(getHunkHeaderSelector(fileName, 1))).toHaveCount(0);
	await expect(unifiedDiffView).toContainText("eighteen changed");
	await expect(unifiedDiffView).not.toContainText("two changed");
});

test("should discard an untracked added file via context menu", async ({ page, gitbutler }) => {
	const fileName = "demo.txt";
	const filePath = join(gitbutler.pathInWorkdir("local-clone/"), fileName);

	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	writeFileSync(filePath, "Hello world\nSecond line\n", "utf-8");
	expect(existsSync(filePath)).toBe(true);

	const fileItem = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await fileItem.click();

	const unifiedDiffView = getByTestId(page, "unified-diff-view");
	await expect(unifiedDiffView).toBeVisible();

	await unifiedDiffView
		.locator('[data-testid="hunk-count-column"]')
		.first()
		.click({ button: "right" });
	await waitForTestId(page, "hunk-context-menu");
	await clickByTestId(page, "hunk-context-menu-discard-change");

	await expect.poll(() => existsSync(filePath)).toBe(false);
	await expect(fileItem).toHaveCount(0);
});

async function commitFileToNewBranch(page: Page, fileName: string, title: string) {
	await clickByTestId(page, "commit-to-new-branch-button");
	const fileItem = getByTestId(page, "uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();
	await fileItem.click();
	await expect(getByTestId(page, "unified-diff-view")).toBeVisible();
	await fillByTestId(page, "commit-drawer-title-input", title);
	await clickByTestId(page, "commit-drawer-action-button");
	await waitForTestIdToNotExist(page, "new-commit-view");
	await expect(commitRow(page, title)).toBeVisible();
}

async function openCommittedFileDiff(page: Page, fileName: string, commitTitle: string) {
	const row = commitRow(page, commitTitle);
	await expect(row).toBeVisible();
	await row.click();

	const fileItem = committedFileItem(page, fileName, commitTitle);
	await fileItem.waitFor({ state: "visible", timeout: 2000 }).catch(async () => {
		await row.click();
		await fileItem.waitFor({ state: "visible" });
	});
	await fileItem.click();

	const unifiedDiffView = getByTestId(page, "unified-diff-view");
	await expect(unifiedDiffView).toBeVisible();
	return unifiedDiffView;
}

function committedFileItem(page: Page, fileName: string, commitTitle: string) {
	return commitRow(page, commitTitle)
		.locator("xpath=..")
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
}

async function uncommitCommittedHunk(
	page: Page,
	fileName: string,
	commitTitle: string,
	hunkIndex: number,
) {
	const unifiedDiffView = await openCommittedFileDiff(page, fileName, commitTitle);
	const header = unifiedDiffView.locator(getHunkHeaderSelector(fileName, hunkIndex)).first();
	await expect(header).toBeVisible();
	await header.click({ button: "right" });
	await waitForTestId(page, "hunk-context-menu");
	await clickByTestId(page, "hunk-context-menu-uncommit-change");
}

async function unselectHunk(fileName: string, unifiedDiffView: Locator, hunkIndex: number) {
	const header = unifiedDiffView.locator(getHunkHeaderSelector(fileName, hunkIndex)).first();
	await expect(header).toBeVisible();
	await header.click();
}

async function unselectHunkLines(fileName: string, unifiedDiffView: Locator, lines: number[]) {
	for (const side of ["left", "right"] as const) {
		for (const line of lines) {
			const locator = unifiedDiffView.locator(getHunkLineSelector(fileName, line, side)).first();
			await expect(locator).toBeVisible();
			await locator.click();
		}
	}
}
