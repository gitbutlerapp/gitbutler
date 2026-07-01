import {
	openCommitDrawer,
	startEditingCommitMessage,
	updateCommitMessage,
	verifyCommitDrawerContent,
	verifyCommitMessageEditor,
	verifyCommitPlaceholderPosition,
} from "../src/commit.ts";
import { stageFirstFile, unstageAllFiles, writeFiles } from "../src/file.ts";
import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	clickByTestId,
	commitRow,
	dragAndDropByLocator,
	getByTestId,
	stack,
	waitForTestIdToNotExist,
} from "../src/util.ts";
import { expect, type Page } from "@playwright/test";
import { copyFileSync, writeFileSync } from "fs";
import { join } from "path";

const FIXTURE_IMAGE_PATH = join(import.meta.dirname, "../fixtures/lesh0.jpg");

test("should be able to amend a file to a commit", async ({ page, gitbutler }) => {
	const filePath = gitbutler.pathInWorkdir("local-clone/b_file");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	// Imported remote branches start out already synced.
	await expect(getByTestId(page, "stack-push-button")).toBeDisabled();

	writeFileSync(filePath, "Hello! this is file b\n", { flag: "w" });

	const fileLocator = getByTestId(page, "file-list-item").filter({ hasText: "b_file" });
	const topCommit = commitRow(page, "branch1:  second commit");

	await dragAndDropByLocator(page, fileLocator, topCommit);
	await clickByTestId(page, "stack-push-button");

	await expect(commitRow(page)).toHaveCount(2);
	await expect(getByTestId(page, "stack-push-button")).toBeDisabled();
});

test("should be able to commit a bunch of times in a row and edit their message", async ({
	page,
	gitbutler,
}) => {
	test.setTimeout(120_000);

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");

	const fileNames = ["file1.txt", "file2.txt", "file3.txt", "file4.txt", "file5.txt", "file6.txt"];
	const filesContent: Record<string, string> = {};
	for (const fileName of fileNames) {
		filesContent[gitbutler.pathInWorkdir(`local-clone/${fileName}`)] = `This is ${fileName}\n`;
	}
	writeFiles(filesContent);

	await openWorkspace(page);

	const TIMES = 3;
	await commitMultipleTimes(TIMES, page);
	await amendCommitMessageMultipleTimes(TIMES - 1, page);
	await startAmendingACommitMessageAndCancel(page, TIMES - 1);
	await startCommittingAndCancel(page, TIMES);
});

test("should be able to commit a binary file", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");

	const targetImagePath = gitbutler.pathInWorkdir("local-clone/lesh0.jpg");
	copyFileSync(FIXTURE_IMAGE_PATH, targetImagePath);

	await openWorkspace(page);

	const fileLocator = getByTestId(page, "file-list-item").filter({ hasText: "lesh0.jpg" });
	await expect(fileLocator).toBeVisible();

	await clickByTestId(page, "start-commit-button");
	await verifyCommitPlaceholderPosition(page);
	await unstageAllFiles(page);

	await fileLocator.locator('input[type="checkbox"]').click();

	const commitTitle = "Add binary image file";
	const commitMessage = "Adding lesh0.jpg to the repository";
	await verifyCommitMessageEditor(page, "", "");
	await updateCommitMessage(page, commitTitle, commitMessage);
	await clickByTestId(page, "commit-drawer-action-button");

	await expect(commitRow(page, commitTitle)).toBeVisible();
	const drawer = await openCommitDrawer(page, commitTitle);
	await verifyCommitDrawerContent(drawer, commitTitle, commitMessage);

	await expect(
		stack(page).getByTestId("file-list-item").filter({ hasText: "lesh0.jpg" }),
	).toBeVisible();
});

test("should be able to commit a git submodule", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await gitbutler.runScript("project-with-remote-branches__add-submodule.sh");

	const gitmodulesFile = getByTestId(page, "file-list-item").filter({ hasText: ".gitmodules" });
	const submoduleDir = getByTestId(page, "file-list-item").filter({ hasText: "my-submodule" });
	await expect(gitmodulesFile).toBeVisible();
	await expect(submoduleDir).toBeVisible();

	await clickByTestId(page, "start-commit-button");
	await verifyCommitPlaceholderPosition(page);
	await unstageAllFiles(page);

	await gitmodulesFile.locator('input[type="checkbox"]').click();
	await submoduleDir.locator('input[type="checkbox"]').click();

	const commitTitle = "Add git submodule";
	const commitMessage = "Adding my-submodule to the repository";
	await verifyCommitMessageEditor(page, "", "");
	await updateCommitMessage(page, commitTitle, commitMessage);
	await clickByTestId(page, "commit-drawer-action-button");

	await expect(commitRow(page, commitTitle)).toBeVisible();
	const drawer = await openCommitDrawer(page, commitTitle);
	await verifyCommitDrawerContent(drawer, commitTitle, commitMessage);

	const stackFiles = stack(page).getByTestId("file-list-item");
	await expect(stackFiles.filter({ hasText: ".gitmodules" })).toBeVisible();
	await expect(stackFiles.filter({ hasText: "my-submodule" })).toBeVisible();
});

/**
 * Commit multiple times in a row, staging only the first file each time.
 */
async function commitMultipleTimes(TIMES: number, page: Page) {
	for (let i = 0; i < TIMES; i++) {
		await clickByTestId(page, "start-commit-button");
		await verifyCommitPlaceholderPosition(page);
		await unstageAllFiles(page);
		await stageFirstFile(page);

		const title = commitTitleFor(i);
		const body = commitDescriptionFor(i);
		await verifyCommitMessageEditor(page, "", "");
		await updateCommitMessage(page, title, body);
		await clickByTestId(page, "commit-drawer-action-button");

		await expect(commitRow(page, title)).toBeVisible();
	}
}

async function startCommittingAndCancel(page: Page, index: number) {
	await clickByTestId(page, "start-commit-button");
	await verifyCommitPlaceholderPosition(page);
	await unstageAllFiles(page);
	await stageFirstFile(page);

	const title = commitTitleFor(index);
	await verifyCommitMessageEditor(page, "", "");
	await updateCommitMessage(page, title, commitDescriptionFor(index));
	await clickByTestId(page, "commit-drawer-cancel-button");

	await expect(commitRow(page, title)).toHaveCount(0);
}

async function amendCommitMessageMultipleTimes(TIMES: number, page: Page) {
	for (let i = 0; i < TIMES; i++) {
		const title = commitTitleFor(i);
		const body = commitDescriptionFor(i);
		const newTitle = amendedTitleFor(i);
		const newBody = amendedDescriptionFor(i);

		const drawer = await openCommitDrawer(page, title);
		await verifyCommitDrawerContent(drawer, title, body);

		await startEditingCommitMessage(page, drawer);
		await verifyCommitMessageEditor(page, title, body);

		await updateCommitMessage(page, newTitle, newBody);
		await clickByTestId(page, "commit-drawer-action-button");

		await waitForTestIdToNotExist(page, "edit-commit-message-box");
		await verifyCommitDrawerContent(drawer, newTitle, newBody);
	}
}

async function startAmendingACommitMessageAndCancel(page: Page, index: number) {
	const title = commitTitleFor(index);
	const body = commitDescriptionFor(index);

	const drawer = await openCommitDrawer(page, title);
	await verifyCommitDrawerContent(drawer, title, body);

	await startEditingCommitMessage(page, drawer);
	await verifyCommitMessageEditor(page, title, body);

	await updateCommitMessage(page, amendedTitleFor(index), amendedDescriptionFor(index));
	await clickByTestId(page, "commit-drawer-cancel-button");

	await waitForTestIdToNotExist(page, "edit-commit-message-box");
	await verifyCommitDrawerContent(drawer, title, body);
}

function commitTitleFor(i: number): string {
	return `Commit number ${i + 1}`;
}

function amendedTitleFor(i: number): string {
	return `Amended Commit number ${i + 1}`;
}

function commitDescriptionFor(i: number): string {
	return `Desc ${i + 1}`;
}

function amendedDescriptionFor(i: number): string {
	return `Amended ${i + 1}`;
}
