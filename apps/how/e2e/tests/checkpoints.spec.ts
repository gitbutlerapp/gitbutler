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

async function cloneRepository(remotePath: string, prefix: string): Promise<string> {
	const parentPath = await createTempDirectory(prefix);
	const repositoryPath = path.join(parentPath, "project");
	await runGit(parentPath, ["clone", remotePath, "project"]);
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

async function currentHead(repositoryPath: string): Promise<string> {
	return await runGit(repositoryPath, ["rev-parse", "HEAD"]);
}

async function branchHead(repositoryPath: string, branchName: string): Promise<string> {
	return await runGit(repositoryPath, ["rev-parse", branchName]);
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

function bookmarkItem(page: Page, index: number) {
	return page.locator("aside ul > li").nth(index);
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

test("creates an opening checkpoint for dirty work on the expected line", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-open-dirty-main-project-");
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath, "notes.md", "initial\n", "Initial");
	await runGit(repositoryPath, ["config", "--local", "how.codingAgent", "codex"]);
	await fs.writeFile(path.join(repositoryPath, "notes.md"), "opening dirty work\n");

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		checkpointQuietMs: "5000",
		checkpointSummary: "Captures opening work",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await expect.poll(async () => await currentBranch(repositoryPath)).toBe("main");
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
		await expect(page.getByText("Captures opening work", { exact: true })).toBeVisible();
		await expect
			.poll(async () => await latestCheckpointMessage(repositoryPath))
			.toBe("Checkpoint: Captures opening work");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("prepares a dirty project opened away from the local trunk fallback", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-open-feature-project-");
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath, "notes.md", "main state\n", "Initial");
	await runGit(repositoryPath, ["switch", "-c", "feature"]);
	await fs.writeFile(path.join(repositoryPath, "notes.md"), "feature dirty work\n");

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		checkpointQuietMs: "5000",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await expect.poll(async () => await currentBranch(repositoryPath)).toBe("main");
		await expect.poll(async () => await fs.readFile(path.join(repositoryPath, "notes.md"), "utf8")).toBe(
			"feature dirty work\n",
		);
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
		await expect(page.getByText("Where you left off")).toBeVisible();
		await expect(page.getByText("Shared starting point")).toBeVisible();
		await expect.poll(async () => await currentHead(repositoryPath)).toBe(
			await branchHead(repositoryPath, "feature"),
		);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("uses the remote HEAD trunk as the expected active line", async ({
	browserName: _browserName,
}, testInfo) => {
	const remotePath = await createBareRepository("how-remote-head-remote-");
	const sourcePath = await createTempDirectory("how-remote-head-source-");
	await runGit(sourcePath, ["init", "-b", "trunk"]);
	await runGit(sourcePath, ["config", "user.name", "How E2E"]);
	await runGit(sourcePath, ["config", "user.email", "how-e2e@example.com"]);
	await createRegularCommit(sourcePath, "notes.md", "shared trunk\n", "Initial");
	await runGit(sourcePath, ["remote", "add", "origin", remotePath]);
	await runGit(sourcePath, ["push", "-u", "origin", "trunk"]);
	await runGit(remotePath, ["symbolic-ref", "HEAD", "refs/heads/trunk"]);
	const repositoryPath = await cloneRepository(remotePath, "how-remote-head-clone-");
	await runGit(repositoryPath, ["switch", "-c", "feature"]);
	await createRegularCommit(repositoryPath, "notes.md", "feature state\n", "Feature");

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		checkpointQuietMs: "5000",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await expect.poll(async () => await currentBranch(repositoryPath)).toBe("trunk");
		await expect.poll(async () => await fs.readFile(path.join(repositoryPath, "notes.md"), "utf8")).toBe(
			"feature state\n",
		);
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
		await expect(page.getByText("Where you left off")).toBeVisible();
		await expect(page.getByText("Shared starting point")).toBeVisible();
		await expect.poll(async () => await branchHead(repositoryPath, "trunk")).toBe(
			await branchHead(repositoryPath, "feature"),
		);
		await expect.poll(async () => await branchHead(repositoryPath, "origin/trunk")).not.toBe(
			await branchHead(repositoryPath, "trunk"),
		);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(sourcePath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("creates the local counterpart for the remote HEAD trunk when it is missing", async ({
	browserName: _browserName,
}, testInfo) => {
	const remotePath = await createBareRepository("how-remote-counterpart-remote-");
	const sourcePath = await createTempDirectory("how-remote-counterpart-source-");
	await runGit(sourcePath, ["init", "-b", "trunk"]);
	await runGit(sourcePath, ["config", "user.name", "How E2E"]);
	await runGit(sourcePath, ["config", "user.email", "how-e2e@example.com"]);
	await createRegularCommit(sourcePath, "notes.md", "shared trunk\n", "Initial");
	await runGit(sourcePath, ["remote", "add", "origin", remotePath]);
	await runGit(sourcePath, ["push", "-u", "origin", "trunk"]);
	await runGit(remotePath, ["symbolic-ref", "HEAD", "refs/heads/trunk"]);
	const repositoryPath = await cloneRepository(remotePath, "how-remote-counterpart-clone-");
	await runGit(repositoryPath, ["switch", "-c", "feature"]);
	await createRegularCommit(repositoryPath, "notes.md", "feature state\n", "Feature");
	await runGit(repositoryPath, ["branch", "--delete", "trunk"]);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		checkpointQuietMs: "5000",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await expect.poll(async () => await currentBranch(repositoryPath)).toBe("trunk");
		await expect.poll(async () => await branchHead(repositoryPath, "trunk")).toBe(
			await branchHead(repositoryPath, "feature"),
		);
		await expect.poll(async () => await fs.readFile(path.join(repositoryPath, "notes.md"), "utf8")).toBe(
			"feature state\n",
		);
		await expect(page.getByText("Where you left off")).toBeVisible();
		await expect(page.getByText("Shared starting point")).toBeVisible();
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(sourcePath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("prepares again after reopening a project that was manually checked out elsewhere", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-reopen-manual-checkout-project-");
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath, "notes.md", "main state\n", "Initial");
	await runGit(repositoryPath, ["switch", "-c", "feature"]);
	await fs.writeFile(path.join(repositoryPath, "notes.md"), "first dirty feature state\n");

	const userDataPath = testInfo.outputPath("user-data");
	const first = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath,
		checkpointQuietMs: "5000",
	});
	try {
		await first.page.getByRole("button", { name: "Open project" }).click();
		await expect(first.page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();
		await expect.poll(async () => await currentBranch(repositoryPath)).toBe("main");
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
	} finally {
		await first.app.close();
	}

	await runGit(repositoryPath, ["switch", "feature"]);
	await fs.writeFile(path.join(repositoryPath, "notes.md"), "manual dirty feature state\n");

	const second = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath,
		checkpointQuietMs: "5000",
	});
	try {
		await expect(second.page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();
		await expect.poll(async () => await currentBranch(repositoryPath)).toBe("main");
		await expect
			.poll(async () => await fs.readFile(path.join(repositoryPath, "notes.md"), "utf8"))
			.toBe("manual dirty feature state\n");
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(2);
		await expect(second.page.getByText("Where you left off").first()).toBeVisible();
		await expect(second.page.getByText("Shared starting point").first()).toBeVisible();
	} finally {
		await second.app.close();
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

		await expect(page.locator("ol li").first()).toBeVisible();
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("creates bookmarks, switches to one, and preserves the previous state", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-bookmark-project-");
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
		await page.getByRole("button", { name: "Bookmark current state" }).first().click();
		await page.getByLabel("Name").fill("Version A");
		await page.getByRole("button", { name: "Save" }).click();
		await expect(page.getByText("Version A")).toBeVisible();
		await expect(
			page.locator("li.checkpoint-message-flash").filter({ hasText: "Version A" }),
		).toBeVisible();
		await expect(page.getByText("current")).toBeVisible();
		await expect(page.locator("aside li.checkpoint-message-flash")).toHaveCount(0);

		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint B\n", 2);
		await expect(page.getByText("current")).toHaveCount(0);

		await page.getByText("Version A").click();
		await expect.poll(async () => await fs.readFile(notesPath, "utf8")).toBe("checkpoint A\n");
		await expect(page.getByText("Version Acurrent")).toBeVisible();
		await expect(page.getByText("Before switching to Version A")).toBeVisible();
		await expect(page.locator("aside li.checkpoint-message-flash")).toHaveCount(0);
		await expect
			.poll(
				async () =>
					await runGit(repositoryPath, [
						"for-each-ref",
						"--format=%(refname)",
						"refs/gitbutler/how/bookmarks",
					]),
			)
			.toContain("refs/gitbutler/how/bookmarks/");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
	}
});

test("keeps bookmarks ordered by update time when switching", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-bookmark-order-project-");
	await initializeGitRepository(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint A\n", 1);
		await page.getByRole("button", { name: "Bookmark current state" }).first().click();
		await page.getByLabel("Name").fill("Version A");
		await page.getByRole("button", { name: "Save" }).click();
		await expect(page.getByText("Version A")).toBeVisible();

		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint B\n", 2);
		await page.getByRole("button", { name: "Bookmark current state" }).first().click();
		await page.getByLabel("Name").fill("Version B");
		await page.getByRole("button", { name: "Save" }).click();
		await expect(bookmarkItem(page, 0)).toContainText("Version B");
		await expect(bookmarkItem(page, 1)).toContainText("Version A");

		await bookmarkItem(page, 1).getByText("Version A").click();
		await expect(bookmarkItem(page, 0)).toContainText("Version B");
		await expect(bookmarkItem(page, 1)).toContainText("Version A");
		await expect(bookmarkItem(page, 1)).toContainText("current");
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

		await expect(page.getByText("Adds notes screen", { exact: true })).toBeVisible();
		await expect(
			page.locator("li.checkpoint-message-flash").filter({ hasText: "Adds notes screen" }),
		).toBeVisible();
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

		await expect(page.locator("ol li").first()).toBeVisible();
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
		await page.getByLabel("Check for shared updates").selectOption(String(30 * 60 * 1000));
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
		await expect
			.poll(
				async () =>
					await runGit(repositoryPath, ["config", "--local", "--get", "how.fetchIntervalMs"]),
			)
			.toBe(String(30 * 60 * 1000));

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

test("logs in, chooses an existing GitHub project, and publishes", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-github-publish-existing-project-");
	const remotePath = await createBareRepository();
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);
	const branchName = await currentBranch(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		githubLogin: "how-test",
		githubRepositories: [
			{
				id: "repo-1",
				nameWithOwner: "how-test/existing-project",
				cloneUrl: remotePath,
				isPrivate: true,
			},
		],
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await page.getByRole("button", { name: "Publish" }).click();
		await expect(page.getByRole("heading", { name: "Publish with GitHub" })).toBeVisible();
		await page.getByRole("button", { name: "Log in to GitHub" }).click();
		await expect(page.getByRole("heading", { name: "Where should this publish?" })).toBeVisible();
		await page.getByRole("button", { name: "Choose existing project" }).click();
		await expect(page.getByRole("heading", { name: "Choose existing project" })).toBeVisible();
		await page.getByRole("button", { name: "how-test/existing-project" }).click();

		await expect(page.getByText("Published just now")).toBeVisible();
		await expect
			.poll(
				async () =>
					await runGit(repositoryPath, [
						"rev-parse",
						"--abbrev-ref",
						"--symbolic-full-name",
						"@{u}",
					]),
			)
			.toBe(`origin/${branchName}`);
		await expect.poll(async () => await runGit(remotePath, ["rev-parse", branchName])).toBeTruthy();
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("logs in, creates a GitHub project, and publishes", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-github-publish-create-project-");
	const remotePath = await createBareRepository();
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);
	const branchName = await currentBranch(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		githubLogin: "how-test",
		githubCreateRepositoryUrl: remotePath,
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await page.getByRole("button", { name: "Publish" }).click();
		await page.getByRole("button", { name: "Log in to GitHub" }).click();
		await page.getByRole("button", { name: "Create GitHub project" }).click();
		await expect(page.getByRole("heading", { name: "Create GitHub project" })).toBeVisible();
		await page.getByRole("button", { name: "Create and publish" }).click();

		await expect(page.getByText("Published just now")).toBeVisible();
		await expect
			.poll(async () => await runGit(repositoryPath, ["remote", "get-url", "origin"]))
			.toBe(remotePath);
		await expect
			.poll(
				async () =>
					await runGit(repositoryPath, [
						"rev-parse",
						"--abbrev-ref",
						"--symbolic-full-name",
						"@{u}",
					]),
			)
			.toBe(`origin/${branchName}`);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("creates a checkpoint before publishing unsaved changes", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-direct-publish-checkpoint-project-");
	const remotePath = await createBareRepository();
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		checkpointQuietMs: "5000",
		githubLogin: "how-test",
		githubCreateRepositoryUrl: remotePath,
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await fs.writeFile(path.join(repositoryPath, "notes.md"), "publish checkpoint\n");
		await page.getByRole("button", { name: "Publish" }).click();
		await page.getByRole("button", { name: "Log in to GitHub" }).click();
		await page.getByRole("button", { name: "Create GitHub project" }).click();
		await page.getByRole("button", { name: "Create and publish" }).click();

		await expect(page.getByText("Published just now")).toBeVisible();
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(1);
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("hides published checkpoints after publish refreshes the shared project", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-published-checkpoints-project-");
	const remotePath = await createBareRepository();
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		githubLogin: "how-test",
		githubCreateRepositoryUrl: remotePath,
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint A\n", 1);
		await createCheckpoint(page, repositoryPath, "notes.md", "checkpoint B\n", 2);
		await expect(page.locator("ol li")).toHaveCount(2);

		await page.getByRole("button", { name: "Publish" }).click();
		await page.getByRole("button", { name: "Log in to GitHub" }).click();
		await page.getByRole("button", { name: "Create GitHub project" }).click();
		await page.getByRole("button", { name: "Create and publish" }).click();

		await expect(page.getByText("Published just now")).toBeVisible();
		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(2);
		await expect(page.locator("ol li")).toHaveCount(0);
		await expect(page.getByText("No checkpoints yet")).toBeVisible();
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("keeps unpublished checkpoints visible after published checkpoints are hidden", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-unpublished-checkpoints-project-");
	const remotePath = await createBareRepository();
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		githubLogin: "how-test",
		githubCreateRepositoryUrl: remotePath,
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await createCheckpoint(page, repositoryPath, "notes.md", "published checkpoint\n", 1);
		await page.getByRole("button", { name: "Publish" }).click();
		await page.getByRole("button", { name: "Log in to GitHub" }).click();
		await page.getByRole("button", { name: "Create GitHub project" }).click();
		await page.getByRole("button", { name: "Create and publish" }).click();
		await expect(page.locator("ol li")).toHaveCount(0);

		await createCheckpoint(page, repositoryPath, "notes.md", "unpublished checkpoint\n", 2);

		await expect.poll(async () => await checkpointCommitCount(repositoryPath)).toBe(2);
		await expect(page.locator("ol li")).toHaveCount(1);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("shows update available when the shared project changes", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-shared-update-project-");
	const remotePath = await createBareRepository();
	const clonePath = await createTempDirectory("how-shared-update-clone-");
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);
	const branchName = await currentBranch(repositoryPath);
	await runGit(repositoryPath, ["remote", "add", "origin", remotePath]);
	await runGit(repositoryPath, ["push", "-u", "origin", `HEAD:${branchName}`]);
	await fs.rm(clonePath, { recursive: true, force: true });
	await runGit(os.tmpdir(), ["clone", remotePath, clonePath]);
	await runGit(clonePath, ["config", "user.name", "How E2E"]);
	await runGit(clonePath, ["config", "user.email", "how-e2e@example.com"]);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		sharedFetchIntervalMs: "100",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await createRegularCommit(clonePath, "remote.md", "remote change\n", "Remote change");
		await runGit(clonePath, ["push"]);

		await expect(page.getByText("Update available")).toBeVisible();
		const publishButton = page.getByRole("button", { name: "Publish" });
		await expect(publishButton).toBeDisabled();
		await expect(publishButton).toHaveAttribute("title", "Update this project before publishing.");
		await expect(page.getByRole("button", { name: "Update project" })).toBeEnabled();
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
		await fs.rm(clonePath, { recursive: true, force: true });
	}
});

test("updates the project by replaying local checkpoints onto the shared project", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-update-project-success-");
	const remotePath = await createBareRepository();
	const clonePath = await createTempDirectory("how-update-project-success-clone-");
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);
	const branchName = await currentBranch(repositoryPath);
	await runGit(repositoryPath, ["remote", "add", "origin", remotePath]);
	await runGit(repositoryPath, ["push", "-u", "origin", `HEAD:${branchName}`]);
	await fs.rm(clonePath, { recursive: true, force: true });
	await runGit(os.tmpdir(), ["clone", remotePath, clonePath]);
	await runGit(clonePath, ["config", "user.name", "How E2E"]);
	await runGit(clonePath, ["config", "user.email", "how-e2e@example.com"]);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		sharedFetchIntervalMs: "100",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await createCheckpoint(page, repositoryPath, "local.md", "local checkpoint\n", 1);
		await createRegularCommit(clonePath, "remote.md", "remote change\n", "Remote change");
		await runGit(clonePath, ["push"]);
		await expect(page.getByText("Update available")).toBeVisible();

		await page.getByRole("button", { name: "Update project" }).click();

		await expect(page.getByText("Updated just now")).toBeVisible();
		await expect(page.getByRole("button", { name: "Publish" })).toBeEnabled();
		await expect(page.locator("ol li")).toHaveCount(1);
		await expect
			.poll(async () => await fs.readFile(path.join(repositoryPath, "local.md"), "utf8"))
			.toBe("local checkpoint\n");
		await expect
			.poll(async () => await fs.readFile(path.join(repositoryPath, "remote.md"), "utf8"))
			.toBe("remote change\n");
		await expect
			.poll(async () =>
				runGit(repositoryPath, ["merge-base", "--is-ancestor", "@{u}", "HEAD"])
					.then(() => true)
					.catch(() => false),
			)
			.toBe(true);
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
		await fs.rm(clonePath, { recursive: true, force: true });
	}
});

test("shows a plain error and leaves the project unchanged when update conflicts", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-update-project-conflict-");
	const remotePath = await createBareRepository();
	const clonePath = await createTempDirectory("how-update-project-conflict-clone-");
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath, "notes.md", "initial\n", "Initial");
	const branchName = await currentBranch(repositoryPath);
	await runGit(repositoryPath, ["remote", "add", "origin", remotePath]);
	await runGit(repositoryPath, ["push", "-u", "origin", `HEAD:${branchName}`]);
	await fs.rm(clonePath, { recursive: true, force: true });
	await runGit(os.tmpdir(), ["clone", remotePath, clonePath]);
	await runGit(clonePath, ["config", "user.name", "How E2E"]);
	await runGit(clonePath, ["config", "user.email", "how-e2e@example.com"]);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		sharedFetchIntervalMs: "100",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();

		await createCheckpoint(page, repositoryPath, "notes.md", "local edit\n", 1);
		const preUpdateHead = await runGit(repositoryPath, ["rev-parse", "HEAD"]);
		await createRegularCommit(clonePath, "notes.md", "remote edit\n", "Remote edit");
		await runGit(clonePath, ["push"]);
		await expect(page.getByText("Update available")).toBeVisible();

		await page.getByRole("button", { name: "Update project" }).click();

		await expect(page.getByText("How could not update this project automatically.")).toBeVisible();
		await expect
			.poll(async () => await runGit(repositoryPath, ["rev-parse", "HEAD"]))
			.toBe(preUpdateHead);
		await expect
			.poll(async () => await fs.readFile(path.join(repositoryPath, "notes.md"), "utf8"))
			.toBe("local edit\n");
		await expect.poll(async () => await runGit(repositoryPath, ["status", "--porcelain"])).toBe("");
		await expect
			.poll(async () =>
				(await fs.readFile(path.join(repositoryPath, "notes.md"), "utf8")).includes("<<<<<<<"),
			)
			.toBe(false);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
		await fs.rm(clonePath, { recursive: true, force: true });
	}
});

test("background shared update failure is soft", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-shared-fetch-failure-project-");
	const remotePath = await createBareRepository();
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);
	await runGit(repositoryPath, ["remote", "add", "origin", remotePath]);
	await runGit(repositoryPath, [
		"push",
		"-u",
		"origin",
		`HEAD:${await currentBranch(repositoryPath)}`,
	]);

	const { app, page } = await launchHowApp({
		projectPath: repositoryPath,
		userDataPath: testInfo.outputPath("user-data"),
		sharedFetchIntervalMs: "100",
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();
		await fs.rm(remotePath, { recursive: true, force: true });

		await expect(page.getByText("Could not check for updates")).toBeVisible();
		await createCheckpoint(page, repositoryPath, "notes.md", "local work after fetch failure\n", 1);
		await expect(page.locator("ol li")).toHaveCount(1);
	} finally {
		await app.close();
		await fs.rm(repositoryPath, { recursive: true, force: true });
		await fs.rm(remotePath, { recursive: true, force: true });
	}
});

test("disables publish while browsing checkpoints", async ({
	browserName: _browserName,
}, testInfo) => {
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

test("shows a plain-language error when the shared project changed", async ({
	browserName: _browserName,
}, testInfo) => {
	const repositoryPath = await createTempDirectory("how-direct-publish-rejected-project-");
	const remotePath = await createBareRepository();
	const clonePath = await createTempDirectory("how-direct-publish-other-clone-");
	await initializeGitRepository(repositoryPath);
	await createRegularCommit(repositoryPath);
	await runGit(repositoryPath, ["remote", "add", "origin", remotePath]);
	await runGit(repositoryPath, [
		"push",
		"-u",
		"origin",
		`HEAD:${await currentBranch(repositoryPath)}`,
	]);
	await runGit(repositoryPath, ["remote", "remove", "origin"]);
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
		githubLogin: "how-test",
		githubRepositories: [
			{
				id: "repo-1",
				nameWithOwner: "how-test/rejected-project",
				cloneUrl: remotePath,
				isPrivate: true,
			},
		],
	});
	try {
		await page.getByRole("button", { name: "Open project" }).click();
		await expect(page.getByRole("heading", { name: pathTitle(repositoryPath) })).toBeVisible();
		await fs.writeFile(path.join(repositoryPath, "local.md"), "local change\n");

		await page.getByRole("button", { name: "Publish" }).click();
		await page.getByRole("button", { name: "Log in to GitHub" }).click();
		await page.getByRole("button", { name: "Choose existing project" }).click();
		await page.getByRole("button", { name: "how-test/rejected-project" }).click();

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
