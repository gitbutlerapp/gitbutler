import { discardFile } from "../src/file.ts";
import { getHunkHeaderSelector, getHunkLineSelector } from "../src/hunk.ts";
import { getBaseURL, startGitButler, type GitButler } from "../src/setup.ts";
import { clickByTestId, fillByTestId, getByTestId, waitForTestId } from "../src/util.ts";
import { expect, Locator, test } from "@playwright/test";
import { existsSync, readFileSync, writeFileSync } from "fs";
import { join, resolve } from "path";

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL(),
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

const BIG_FILE_PATH_BEFORE = resolve(import.meta.dirname, "../fixtures/big-file_before.md");
const BIG_FILE_PATH_AFTER = resolve(import.meta.dirname, "../fixtures/big-file_after.md");
const BIG_FILE_PATH_AFTER_SMALL_CHANGES = resolve(
	import.meta.dirname,
	"../fixtures/big-file_after-small-changes.md",
);

test("should be able to select the hunks correctly in a complex file", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	const projectName = "my-new-project";
	const fileName = "big-file.md";

	const projectPath = gitbutler.pathInWorkdir(projectName + "/");
	const bigFilePath = join(projectPath, fileName);
	const contentBefore = readFileSync(BIG_FILE_PATH_BEFORE, "utf-8");
	const contentAfter = readFileSync(BIG_FILE_PATH_AFTER, "utf-8");

	await gitbutler.runScript("project-with-remote-branches.sh");
	// Add the big file on the remote base
	await gitbutler.runScript("project-with-remote-branches__commit-file-into-remote-base.sh", [
		"Create big file with complex changes",
		fileName,
		contentBefore,
	]);
	// Clone into a new project
	await gitbutler.runScript("project-with-remote-branches__clone-into-new-project.sh", [
		projectName,
	]);
	// Delete the other project to avoid having to switch between them
	await gitbutler.runScript("project-with-remote-branches__delete-project.sh", ["local-clone"]);

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	// Make the changes to the big file in the local project
	writeFileSync(bigFilePath, contentAfter, "utf-8");

	// Start the commit process
	await clickByTestId(page, "commit-to-new-branch-button");

	// The file should appear on the uncommitted changes area
	const uncommittedChangesList = getByTestId(page, "uncommitted-changes-file-list");
	let fileItem = uncommittedChangesList.getByTestId("file-list-item").filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();
	await fileItem.click();

	// The unified diff view should be visible
	const unifiedDiffView = getByTestId(page, "unified-diff-view");
	await expect(unifiedDiffView).toBeVisible();

	let leftLines = [1, 5, 9, 11, 13, 19, 23];
	let rightLines = [1, 5, 9, 11, 13, 19, 23];

	// Unselect a couple of lines
	await unselectHunkLines(fileName, unifiedDiffView, leftLines, rightLines);

	// Commit the changes
	await fillByTestId(page, "commit-drawer-title-input", "Partial commit: Part 1");
	await clickByTestId(page, "commit-drawer-action-button");

	// Start the commit process
	await clickByTestId(page, "start-commit-button");

	fileItem = uncommittedChangesList.getByTestId("file-list-item").filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();
	await fileItem.click();

	leftLines = [1, 5, 9, 11];
	rightLines = [1, 5, 9, 11];

	// Unselect a couple of lines
	await unselectHunkLines(fileName, unifiedDiffView, leftLines, rightLines);

	// Commit the changes
	await fillByTestId(page, "commit-drawer-title-input", "Partial commit: Part 2");
	await clickByTestId(page, "commit-drawer-action-button");

	// Start the commit process
	await clickByTestId(page, "start-commit-button");

	// Commit the changes
	await fillByTestId(page, "commit-drawer-title-input", "Full commit: Part 3");
	await clickByTestId(page, "commit-drawer-action-button");

	// Verify the commits
	const commits = getByTestId(page, "commit-row");
	await expect(commits).toHaveCount(3);
});

test("should unselect a complete hunk", async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	const projectName = "hunk-unselect-project";
	const fileName = "hunk-file.md";

	const projectPath = gitbutler.pathInWorkdir(projectName + "/");
	const filePath = join(projectPath, fileName);

	const contentBefore = readFileSync(BIG_FILE_PATH_BEFORE, "utf-8");
	const contentAfter = readFileSync(BIG_FILE_PATH_AFTER_SMALL_CHANGES, "utf-8");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await gitbutler.runScript("project-with-remote-branches__commit-file-into-remote-base.sh", [
		"Initial file for hunk unselect",
		fileName,
		contentBefore,
	]);
	await gitbutler.runScript("project-with-remote-branches__clone-into-new-project.sh", [
		projectName,
	]);
	await gitbutler.runScript("project-with-remote-branches__delete-project.sh", ["local-clone"]);

	await page.goto("/");

	await waitForTestId(page, "workspace-view");

	writeFileSync(filePath, contentAfter, "utf-8");

	await clickByTestId(page, "commit-to-new-branch-button");

	const uncommittedChangesList = getByTestId(page, "uncommitted-changes-file-list");
	const fileItem = uncommittedChangesList
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();
	await fileItem.click();

	const unifiedDiffView = getByTestId(page, "unified-diff-view");
	await expect(unifiedDiffView).toBeVisible();

	// Unselect the first hunk (hunkIndex = 0)
	await unselectHunk(fileName, unifiedDiffView, 0);

	// Commit the changes
	await fillByTestId(page, "commit-drawer-title-input", "Partial commit: Part 1");
	await clickByTestId(page, "commit-drawer-action-button");

	// Discard the remaining changes
	await discardFile(fileName, page);

	// Verify the file has the expected content
	const finalContent = readFileSync(filePath, "utf-8");
	expect(finalContent).toMatchSnapshot();
});

test("should discard an untracked added file via context menu", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	const fileName = "demo.txt";
	const projectPath = gitbutler.pathInWorkdir("local-clone/");
	const filePath = join(projectPath, fileName);

	await gitbutler.runScript("project-with-remote-branches.sh");

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	// Create an untracked file.
	writeFileSync(filePath, "Hello world\nSecond line\n", "utf-8");
	expect(existsSync(filePath)).toBe(true);

	// The file should appear on the uncommitted changes area
	const uncommittedChangesList = getByTestId(page, "uncommitted-changes-file-list");
	const fileItem = uncommittedChangesList
		.getByTestId("file-list-item")
		.filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();
	await fileItem.click();

	// The unified diff view should be visible
	const unifiedDiffView = getByTestId(page, "unified-diff-view");
	await expect(unifiedDiffView).toBeVisible();

	// Open the hunk context menu for the added file and discard it.
	await unifiedDiffView
		.locator('[data-testid="hunk-count-column"]')
		.first()
		.click({ button: "right" });
	await waitForTestId(page, "hunk-context-menu");
	await clickByTestId(page, "hunk-context-menu-discard-change");

	await expect.poll(() => existsSync(filePath)).toBe(false);
	await expect(fileItem).toHaveCount(0);
	await expect(getByTestId(page, "workspace-view")).toBeVisible();
});

async function unselectHunk(fileName: string, unifiedDiffView: Locator, hunkIndex: number) {
	const headerSelector = getHunkHeaderSelector(fileName, hunkIndex);
	const header = unifiedDiffView.locator(headerSelector).first();
	await expect(header).toBeVisible();
	await header.click();
}

async function unselectHunkLines(
	fileName: string,
	unifiedDiffView: Locator,
	leftLines: number[],
	rightLines: number[],
) {
	for (const line of leftLines) {
		const leftSelector = getHunkLineSelector(fileName, line, "left");
		const leftLine = unifiedDiffView.locator(leftSelector).first();
		await expect(leftLine).toBeVisible();
		await leftLine.click();
	}

	for (const line of rightLines) {
		const rightSelector = getHunkLineSelector(fileName, line, "right");
		const rightLine = unifiedDiffView.locator(rightSelector).first();
		await expect(rightLine).toBeVisible();
		await rightLine.click();
	}
}
