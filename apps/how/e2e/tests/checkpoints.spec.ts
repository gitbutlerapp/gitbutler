import {
	checkpointCommitCount,
	checkpointCommitIds,
	initializeGitRepository,
	pathTitle,
	runGit,
} from "../src/git";
import { launchHowApp } from "../src/how-app";
import { expect, test } from "@playwright/test";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

async function createTempDirectory(prefix: string): Promise<string> {
	return await fs.mkdtemp(path.join(os.tmpdir(), prefix));
}

test("opens an existing Git project", async ({ browserName: _browserName }, testInfo) => {
	const repositoryPath = await createTempDirectory("how-existing-project-");
	await initializeGitRepository(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();

		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();
		await expect(page.getByRole("heading", { exact: true, name: "how" })).toHaveCount(0);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("starts a new folder", async ({ browserName: _browserName }, testInfo) => {
	const projectPath = await createTempDirectory("how-new-project-");

	const { app, page } = await launchHowApp({
		projectPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Start project" }).click();

		await expect(page.getByRole("heading", { name: pathTitle(projectPath) })).toBeVisible();
		await expect.poll(async () => await fs.stat(path.join(projectPath, ".git")).then(() => true)).toBe(
			true,
		);
	} finally {
		await app.close();
		await fs.rm(projectPath, { recursive: true, force: true });
	}
});

test("creates and shows a checkpoint", async ({ browserName: _browserName }, testInfo) => {
	const repositoryPath = await createTempDirectory("how-checkpoint-project-");
	await initializeGitRepository(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await fs.writeFile(path.join(repositoryPath, "notes.md"), "first checkpoint\n");

		await expect(page.getByText(/^Checkpoint: /)).toBeVisible();
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("goes back to an earlier checkpoint", async ({ browserName: _browserName }, testInfo) => {
	const repositoryPath = await createTempDirectory("how-restore-project-");
	await initializeGitRepository(repositoryPath);
	const notesPath = path.join(repositoryPath, "notes.md");

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await fs.writeFile(notesPath, "first checkpoint\n");
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);

		const firstCheckpointId = (await checkpointCommitIds(repositoryPath))[0];
		expect(firstCheckpointId).toBeTruthy();

		await fs.writeFile(notesPath, "second checkpoint\n");
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(2);

		const olderCheckpointItem = page.locator("li").filter({ hasText: /^Checkpoint: / }).nth(1);
		await olderCheckpointItem.hover();
		page.once("dialog", (dialog) => void dialog.accept());
		await olderCheckpointItem.getByRole("button", { name: "go back" }).click();

		await expect(page.getByText("Went back")).toBeVisible();
		await expect.poll(async () => await fs.readFile(notesPath, "utf8")).toBe("first checkpoint\n");
		await expect.poll(async () => await runGit(repositoryPath, ["rev-parse", "HEAD"])).toBe(
			firstCheckpointId,
		);
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});
