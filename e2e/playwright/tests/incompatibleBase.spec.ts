import { getBaseURL, type GitButler, startGitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	clickByTestId,
	dragAndDropByLocator,
	fillByTestId,
	getByTestId,
	textEditorFillByTestId,
	waitForTestId,
} from "../src/util.ts";
import { expect, type Page } from "@playwright/test";
import { writeFileSync } from "fs";

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL(),
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

const SHARED_FILE_CONTENT = [
	"alpha",
	"bravo",
	"charlie",
	"delta",
	"echo",
	"foxtrot",
	"golf",
	"hotel",
	"india",
	"JULIET-MODIFIED-BY-USER",
]
	.join("\n")
	.concat("\n");

/**
 * Set up two stacks with different merge bases and write a worktree change
 * to shared_file. Returns the file locator for the uncommitted change.
 */
async function setupDifferentBaseStacks(gitbutler: GitButler, page: Page) {
	await gitbutler.runScript("project-with-different-base-stacks.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["old-stack", "local-clone"]);
	await gitbutler.runScript("apply-upstream-branch.sh", ["new-stack", "local-clone"]);

	await page.goto("/");
	await waitForTestId(page, "workspace-view");

	const stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(2);

	// Write a change to shared_file. old-stack's base has "juliet",
	// new-stack's has "JULIET-UPSTREAM".
	const filePath = gitbutler.pathInWorkdir("local-clone/shared_file");
	writeFileSync(filePath, SHARED_FILE_CONTENT);

	const fileLocator = page
		.getByTestId("uncommitted-changes-file-list")
		.getByTestId("file-list-item")
		.filter({ hasText: "shared_file" });
	await expect(fileLocator).toBeVisible();

	return { stacks, fileLocator };
}

test("amend: should show incompatible-base rejection when amending wrong-base stack", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	const { fileLocator } = await setupDifferentBaseStacks(gitbutler, page);

	// Drag the file onto old-stack's commit to amend it.
	// shared_file has a different base version there → IncompatibleBase.
	const oldStackCommit = getByTestId(page, "commit-row").filter({
		hasText: "old-stack: add other_file",
	});
	await expect(oldStackCommit).toBeVisible();

	await dragAndDropByLocator(page, fileLocator, oldStackCommit);

	// The commit-failed modal should appear with "Incompatible branch base"
	const modal = getByTestId(page, "global-modal-commit-failed");
	await expect(modal).toBeVisible();
	await expect(modal).toContainText("Incompatible branch base");
	await expect(modal).not.toContainText("Cherry-pick merge conflict");

	await clickByTestId(page, "global-modal-action-button");
	await expect(modal).not.toBeVisible();
});

test("commit: should show incompatible-base rejection when creating commit on wrong-base stack", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	const { stacks } = await setupDifferentBaseStacks(gitbutler, page);

	// Click the commit button on old-stack specifically.
	const oldStack = stacks.filter({ hasText: "old-stack" });
	await expect(oldStack).toBeVisible();
	await oldStack.getByTestId("start-commit-button").click();

	// Fill in a commit message and submit
	await waitForTestId(page, "new-commit-view");
	await fillByTestId(page, "commit-drawer-title-input", "Test commit on wrong stack");
	await textEditorFillByTestId(page, "commit-drawer-description-input", "This should be rejected");
	await clickByTestId(page, "commit-drawer-action-button");

	// The commit-failed modal should appear with "Incompatible branch base"
	const modal = getByTestId(page, "global-modal-commit-failed");
	await expect(modal).toBeVisible();
	await expect(modal).toContainText("Incompatible branch base");
	await expect(modal).not.toContainText("Cherry-pick merge conflict");

	await clickByTestId(page, "global-modal-action-button");
	await expect(modal).not.toBeVisible();
});
