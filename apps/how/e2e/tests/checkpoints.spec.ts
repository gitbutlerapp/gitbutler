import {
	checkpointCommitCount,
	checkpointCommitIds,
	initializeGitRepository,
	latestCheckpointMessage,
	pathTitle,
	runGit,
} from "../src/git";
import { launchHowApp } from "../src/how-app";
import { expect, test, type Page } from "@playwright/test";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

async function createTempDirectory(prefix: string): Promise<string> {
	return await fs.mkdtemp(path.join(os.tmpdir(), prefix));
}

async function createBareRepository(prefix = "how-publish-remote-"): Promise<string> {
	const repositoryPath = await createTempDirectory(prefix);
	await runGit(repositoryPath, ["init", "--bare"]);
	return repositoryPath;
}

async function createRegularCommit(
	repositoryPath: string,
	fileName = "readme.md",
	contents = "hello\n",
	message = "Initial",
): Promise<void> {
	await fs.writeFile(path.join(repositoryPath, fileName), contents);
	await runGit(repositoryPath, ["add", "--all"]);
	await runGit(repositoryPath, ["commit", "--no-gpg-sign", "--message", message]);
}

async function currentBranch(repositoryPath: string): Promise<string> {
	return await runGit(repositoryPath, ["branch", "--show-current"]);
}

async function createCheckpoint(
	page: Page,
	repositoryPath: string,
	fileName: string,
	contents: string,
	expectedCount: number,
): Promise<void> {
	await fs.writeFile(path.join(repositoryPath, fileName), contents);
	await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(expectedCount);
	await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
	await page.waitForTimeout(350);
}

function checkpointItem(page: Page, index: number) {
	return page.locator("ol > li").nth(index);
}

async function viewCheckpoint(
	page: Page,
	index: number,
	options: { waitForBrowsing?: boolean } = {},
): Promise<void> {
	const item = checkpointItem(page, index);
	await item.hover();
	await item.getByRole("button", { name: "view" }).click();
	if (options.waitForBrowsing ?? true)
		await expect(page.getByText("Browsing checkpoint")).toBeVisible();
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
		await expect
			.poll(async () => await fs.stat(path.join(projectPath, ".git")).then(() => true))
			.toBe(true);
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

test("uses a coding agent summary for checkpoint titles and commit bodies", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-ai-checkpoint-project-");
	await initializeGitRepository(repositoryPath);
	await runGit(repositoryPath, ["config", "--local", "how.codingAgent", "codex"]);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		checkpointSummary: "Adds notes screen\nStores the first note body.",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await fs.writeFile(path.join(repositoryPath, "notes.md"), "first checkpoint\n");

		await expect(page.getByText("Checkpoint: Adds notes screen")).toBeVisible();
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
		await expect
			.poll(async () => await latestCheckpointMessage(repositoryPath))
			.toBe("Checkpoint: Adds notes screen\n\nStores the first note body.");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("falls back to the date title when checkpoint summarization is too slow", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-ai-timeout-project-");
	await initializeGitRepository(repositoryPath);
	await runGit(repositoryPath, ["config", "--local", "how.codingAgent", "claude"]);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		checkpointSummary: "Adds slow summary",
		checkpointSummaryDelayMs: "200",
		checkpointSummaryTimeoutMs: "50",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await fs.writeFile(path.join(repositoryPath, "notes.md"), "timeout checkpoint\n");

		await expect(page.getByText(/^Checkpoint: /)).toBeVisible();
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
		expect(await latestCheckpointMessage(repositoryPath)).not.toContain("Adds slow summary");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("does not show another save after an autosave settles", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-save-flicker-project-");
	await initializeGitRepository(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await fs.writeFile(path.join(repositoryPath, "notes.md"), "first checkpoint\n");

		await expect(page.getByText("Saved just now")).toBeVisible();
		await page.evaluate(() => {
			const pageWindow = window as typeof window & {
				howSaveEvents?: Array<{ saveState: string; message: string | null }>;
				howSaveEventsUnsubscribe?: () => void;
			};
			pageWindow.howSaveEvents = [];
			pageWindow.howSaveEventsUnsubscribe?.();
			pageWindow.howSaveEventsUnsubscribe = window.how.onStatus((status) => {
				pageWindow.howSaveEvents?.push({
					saveState: status.saveState,
					message: status.message,
				});
			});
		});

		await page.waitForTimeout(900);
		const saveEvents = await page.evaluate(() => {
			const pageWindow = window as typeof window & {
				howSaveEvents?: Array<{ saveState: string; message: string | null }>;
				howSaveEventsUnsubscribe?: () => void;
			};
			pageWindow.howSaveEventsUnsubscribe?.();
			return pageWindow.howSaveEvents ?? [];
		});

		expect(saveEvents).not.toContainEqual(
			expect.objectContaining({
				saveState: "pending",
			}),
		);
		expect(saveEvents).not.toContainEqual(
			expect.objectContaining({
				saveState: "saving",
			}),
		);
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("saves project settings to local Git config and applies debounce immediately", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-settings-project-");
	await initializeGitRepository(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		checkpointQuietMs: "5000",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await page.getByRole("link", { name: "Project settings" }).click();
		await expect(page.getByRole("heading", { name: "Project settings" })).toBeVisible();

		await page.getByLabel("Save delay").fill("1");
		await page.getByText("Claude", { exact: true }).click();
		await page.getByRole("button", { name: "Save" }).click();
		await expect(page.getByText("Saved")).toBeVisible();

		await expect
			.poll(
				async () =>
					await runGit(repositoryPath, ["config", "--local", "--get", "how.checkpointDebounceMs"]),
			)
			.toBe("1000");
		await expect
			.poll(
				async () => await runGit(repositoryPath, ["config", "--local", "--get", "how.codingAgent"]),
			)
			.toBe("claude");

		await page.getByRole("link", { name: "Back" }).click();
		await fs.writeFile(path.join(repositoryPath, "notes.md"), "settings debounce\n");
		await page.waitForTimeout(500);
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(0);
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("configures direct publish and pushes to an existing remote", async (
	{ browserName: _browserName },
	testInfo,
) => {
	const repositoryPath = await createTempDirectory("how-direct-publish-project-");
	const remotePath = await createBareRepository();
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);
	await runGit(repositoryPath, ["remote", "add", "origin", remotePath]);
	const branchName = await currentBranch(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await page.getByRole("button", { name: "Publish" }).click();
		await expect(page.getByRole("heading", { name: "How should this project publish?" })).toBeVisible();
		await expect(page.getByText("Review before publishing")).toBeVisible();
		await expect(page.getByRole("radio").nth(1)).toBeDisabled();
		await page.getByRole("button", { name: "Continue" }).click();

		await expect(page.getByText("Published just now")).toBeVisible();
		await expect
			.poll(async () => await runGit(repositoryPath, ["config", "--local", "--get", "how.publishMode"]))
			.toBe("direct");
		await expect
			.poll(async () => await runGit(repositoryPath, ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]))
			.toBe(`origin/${branchName}`);
		await expect.poll(async () => await runGit(remotePath, ["rev-parse", branchName])).toBeTruthy();
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("asks for a project destination when publishing without a remote", async (
	{ browserName: _browserName },
	testInfo,
) => {
	const repositoryPath = await createTempDirectory("how-direct-publish-destination-project-");
	const remotePath = await createBareRepository();
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);
	const branchName = await currentBranch(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await page.getByRole("button", { name: "Publish" }).click();
		await page.getByRole("button", { name: "Continue" }).click();
		await expect(page.getByRole("heading", { name: "Add a project destination" })).toBeVisible();
		await page.getByLabel("Project destination URL").fill(remotePath);
		await page.getByRole("button", { name: "Add destination and publish" }).click();

		await expect(page.getByText("Published just now")).toBeVisible();
		await expect.poll(async () => await runGit(repositoryPath, ["remote", "get-url", "origin"])).toBe(
			remotePath,
		);
		await expect
			.poll(async () => await runGit(repositoryPath, ["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]))
			.toBe(`origin/${branchName}`);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("creates a checkpoint before publishing unsaved changes", async (
	{ browserName: _browserName },
	testInfo,
) => {
	const repositoryPath = await createTempDirectory("how-direct-publish-checkpoint-project-");
	const remotePath = await createBareRepository();
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);
	await runGit(repositoryPath, ["remote", "add", "origin", remotePath]);
	await runGit(repositoryPath, ["config", "--local", "how.publishMode", "direct"]);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		checkpointQuietMs: "5000",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await fs.writeFile(path.join(repositoryPath, "notes.md"), "publish checkpoint\n");
		await page.getByRole("button", { name: "Publish" }).click();

		await expect(page.getByText("Published just now")).toBeVisible();
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("disables publish while browsing checkpoints", async (
	{ browserName: _browserName },
	testInfo,
) => {
	const repositoryPath = await createTempDirectory("how-direct-publish-browsing-project-");
	await initializeGitRepository(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();
		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint A\n", 1);
		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint B\n", 2);

		await viewCheckpoint(page, 1);

		await expect(page.getByRole("button", { name: "Publish" })).toBeDisabled();
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("shows a plain-language error when the shared project changed", async (
	{ browserName: _browserName },
	testInfo,
) => {
	const repositoryPath = await createTempDirectory("how-direct-publish-rejected-project-");
	const remotePath = await createBareRepository();
	const clonePath = await createTempDirectory("how-direct-publish-other-clone-");
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);
	await runGit(repositoryPath, ["remote", "add", "origin", remotePath]);
	await runGit(repositoryPath, ["push", "-u", "origin", `HEAD:${await currentBranch(repositoryPath)}`]);
	await runGit(repositoryPath, ["config", "--local", "how.publishMode", "direct"]);
	await fs.rm(clonePath, { recursive: true, force: true });
	await runGit(os.tmpdir(), ["clone", remotePath, clonePath]);
	await runGit(clonePath, ["config", "user.name", "How E2E"]);
	await runGit(clonePath, ["config", "user.email", "how-e2e@example.com"]);
	await createRegularCommit(clonePath, "remote.md", "remote change\n", "Remote change");
	await runGit(clonePath, ["push"]);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		checkpointQuietMs: "5000",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();
		await fs.writeFile(path.join(repositoryPath, "local.md"), "local change\n");

		await page.getByRole("button", { name: "Publish" }).click();

		await expect(
			page.getByText("The shared project has changes How cannot publish over yet."),
		).toBeVisible();
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
		await fs.rm(clonePath, { recursive: true, force: true });
	}
});

test("browses checkpoints and continues from the selected one", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-browse-project-");
	await initializeGitRepository(repositoryPath);
	const notesPath = path.join(repositoryPath, "notes.md");

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint A\n", 1);
		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint B\n", 2);
		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint C\n", 3);

		const [, checkpointB, checkpointA] = await checkpointCommitIds(repositoryPath);
		expect(checkpointA).toBeTruthy();
		expect(checkpointB).toBeTruthy();

		await viewCheckpoint(page, 2);
		await expect(page.getByText("Browsing checkpoint")).toBeVisible();
		await expect(page.getByText("viewing", { exact: true })).toBeVisible();
		await expect(page.locator("ol > li")).toHaveCount(3);
		await expect.poll(async () => await fs.readFile(notesPath, "utf8")).toBe("checkpoint A\n");

		await viewCheckpoint(page, 1, { waitForBrowsing: false });
		await expect.poll(async () => await fs.readFile(notesPath, "utf8")).toBe("checkpoint B\n");

		await page.getByRole("button", { name: "Continue from here" }).click();
		await expect(page.getByText("Saved")).toBeVisible();
		await expect
			.poll(async () => await runGit(repositoryPath, ["rev-parse", "HEAD"]))
			.toBe(checkpointB);
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("enters browsing without saving first when there are no changes", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-browse-clean-fast-project-");
	await initializeGitRepository(repositoryPath);
	await runGit(repositoryPath, ["config", "--local", "how.codingAgent", "codex"]);
	const userDataPath = testInfo.outputPath("user-data");

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath,
		checkpointSummary: "Slow summary",
		checkpointSummaryDelayMs: "3000",
		checkpointSummaryTimeoutMs: "3000",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint A\n", 1);
		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint B\n", 2);

		await viewCheckpoint(page, 1);

		await expect(page.getByText("Browsing checkpoint")).toBeVisible();
	} finally {
		await app.close();
	}

	const log = await fs.readFile(path.join(userDataPath, "how.log"), "utf8");
	expect(log).toContain("Skipping checkpoint before browsing because there are no changes");
	expect(log).not.toContain("Creating checkpoint before browsing");
	await fs.rm(repositoryPath, { recursive: true, force: true });
});

test("pauses autosave while browsing and requires leaving dirty changes before moving", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-browse-dirty-project-");
	await initializeGitRepository(repositoryPath);
	const notesPath = path.join(repositoryPath, "notes.md");

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint A\n", 1);
		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint B\n", 2);
		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint C\n", 3);

		await viewCheckpoint(page, 2);
		await fs.writeFile(notesPath, "dirty browsing edit\n");
		await expect(page.getByText("Changes made while browsing")).toBeVisible();
		await page.waitForTimeout(250);
		await expect(page.locator("ol > li")).toHaveCount(3);
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);

		await viewCheckpoint(page, 1, { waitForBrowsing: false });
		await expect(page.getByRole("heading", { name: "Leave changes?" })).toBeVisible();
		await page.getByRole("button", { name: "Cancel" }).click();
		await expect
			.poll(async () => await fs.readFile(notesPath, "utf8"))
			.toBe("dirty browsing edit\n");

		await viewCheckpoint(page, 1, { waitForBrowsing: false });
		await page.getByRole("button", { name: "Leave changes" }).click();
		await expect(page.getByText("Browsing checkpoint")).toBeVisible();
		await expect.poll(async () => await fs.readFile(notesPath, "utf8")).toBe("checkpoint B\n");
		await expect(page.locator("ol > li")).toHaveCount(3);
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(2);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("returns to latest from clean browsing", async ({ browserName: _browserName }, testInfo) => {
	const repositoryPath = await createTempDirectory("how-browse-return-project-");
	await initializeGitRepository(repositoryPath);
	const notesPath = path.join(repositoryPath, "notes.md");

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint A\n", 1);
		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint B\n", 2);
		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint C\n", 3);

		const [checkpointC] = await checkpointCommitIds(repositoryPath);
		await viewCheckpoint(page, 2);
		await page.getByRole("button", { name: "Return to latest" }).click();

		await expect(page.getByText("Returned to latest")).toBeVisible();
		await expect.poll(async () => await fs.readFile(notesPath, "utf8")).toBe("checkpoint C\n");
		await expect
			.poll(async () => await runGit(repositoryPath, ["rev-parse", "HEAD"]))
			.toBe(checkpointC);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("resumes dirty browsing after restart", async ({ browserName: _browserName }, testInfo) => {
	const repositoryPath = await createTempDirectory("how-browse-restart-project-");
	await initializeGitRepository(repositoryPath);
	const notesPath = path.join(repositoryPath, "notes.md");
	const userDataPath = testInfo.outputPath("user-data");

	const firstRun = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath,
	});
	try {
		await firstRun.page.getByRole("button", { name: "Open project" }).click();
		await expect(
			firstRun.page.getByRole("heading", { name: pathTitle(repositoryPath) }),
		).toBeVisible();

		await createCheckpoint(firstRun.page, repositoryPath, "notes.md", "checkpoint A\n", 1);
		await createCheckpoint(firstRun.page, repositoryPath, "notes.md", "checkpoint B\n", 2);
		await viewCheckpoint(firstRun.page, 1);
		await fs.writeFile(notesPath, "dirty browsing survives restart\n");
		await expect(firstRun.page.getByText("Changes made while browsing")).toBeVisible();
	} finally {
		await firstRun.app.close();
	}

	const secondRun = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath,
	});
	try {
		await expect(
			secondRun.page.getByRole("heading", { name: pathTitle(repositoryPath) }),
		).toBeVisible();
		await expect(secondRun.page.getByText("Changes made while browsing")).toBeVisible();
		await expect(secondRun.page.getByText("viewing", { exact: true })).toBeVisible();
		await expect
			.poll(async () => await fs.readFile(notesPath, "utf8"))
			.toBe("dirty browsing survives restart\n");
		await secondRun.page.waitForTimeout(250);
		await expect(secondRun.page.locator("ol > li")).toHaveCount(2);
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
	} finally {
		await secondRun.app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});
