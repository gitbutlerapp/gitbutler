import { checkpointMessageForStagedChanges } from "../../../electron/src/checkpoint-summarizer";
import {
	createCheckpointCommit,
	discoverRepository,
	listCheckpointCommits,
	publishDirect,
	readPublishMode,
	readProjectSettings,
	resetToCommit,
	writePublishMode,
	writeProjectSettings,
	type GitRepository,
} from "../../../electron/src/git";
import { describe, expect, test } from "vitest";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

async function runGit(args: Array<string>, options: { cwd?: string } = {}): Promise<string> {
	const { stdout } = await execFileAsync("git", args, {
		cwd: options.cwd,
		maxBuffer: 10 * 1024 * 1024,
	});
	return stdout.trim();
}

async function createTestRepository(): Promise<GitRepository> {
	const repositoryPath = await fs.mkdtemp(path.join(os.tmpdir(), "how-git-test-"));
	await runGit(["init"], { cwd: repositoryPath });
	await runGit(["config", "user.name", "How Test"], { cwd: repositoryPath });
	await runGit(["config", "user.email", "how-test@example.com"], { cwd: repositoryPath });
	return await discoverRepository(repositoryPath);
}

async function createBareRepository(): Promise<string> {
	const repositoryPath = await fs.mkdtemp(path.join(os.tmpdir(), "how-bare-test-"));
	await runGit(["init", "--bare"], { cwd: repositoryPath });
	return repositoryPath;
}

async function createRegularCommit(repository: GitRepository): Promise<void> {
	await fs.writeFile(path.join(repository.worktreePath, "readme.md"), "hello\n");
	await runGit(["add", "--all"], { cwd: repository.worktreePath });
	await runGit(["commit", "--no-gpg-sign", "--message", "Initial"], {
		cwd: repository.worktreePath,
	});
}

describe("git helpers", () => {
	test("lists checkpoint commits by their message prefix", async () => {
		const repository = await createTestRepository();

		await fs.writeFile(path.join(repository.worktreePath, "readme.md"), "hello\n");
		await createCheckpointCommit(repository.id, "Checkpoint: Jun 13, 09:30");

		await fs.writeFile(path.join(repository.worktreePath, "readme.md"), "hello again\n");
		await runGit(["add", "--all"], { cwd: repository.worktreePath });
		await runGit(["commit", "--no-gpg-sign", "--message", "Regular commit"], {
			cwd: repository.worktreePath,
		});

		const checkpoints = await listCheckpointCommits(repository.id, 10);

		expect(checkpoints).toHaveLength(1);
		expect(checkpoints[0]?.title).toBe("Checkpoint: Jun 13, 09:30");
	});

	test("resets the repository to a selected commit", async () => {
		const repository = await createTestRepository();
		const readmePath = path.join(repository.worktreePath, "readme.md");

		await fs.writeFile(readmePath, "first\n");
		const firstCommitId = await createCheckpointCommit(repository.id, "Checkpoint: first");
		await fs.writeFile(readmePath, "second\n");
		await createCheckpointCommit(repository.id, "Checkpoint: second");

		expect(firstCommitId).toBeTruthy();
		await resetToCommit(repository.id, firstCommitId ?? "");

		expect(await fs.readFile(readmePath, "utf8")).toBe("first\n");
		expect(await runGit(["rev-parse", "HEAD"], { cwd: repository.worktreePath })).toBe(
			firstCommitId,
		);
	});

	test("reads and writes How project settings in local Git config", async () => {
		const repository = await createTestRepository();

		expect(await readProjectSettings(repository.id)).toEqual({
			checkpointDebounceMs: 10_000,
			codingAgent: "none",
		});

		await writeProjectSettings(repository.id, {
			checkpointDebounceMs: 1_000,
			codingAgent: "claude",
		});

		expect(
			await runGit(["config", "--local", "--get", "how.checkpointDebounceMs"], {
				cwd: repository.worktreePath,
			}),
		).toBe("1000");
		expect(
			await runGit(["config", "--local", "--get", "how.codingAgent"], {
				cwd: repository.worktreePath,
			}),
		).toBe("claude");
		expect(await readProjectSettings(repository.id)).toEqual({
			checkpointDebounceMs: 1_000,
			codingAgent: "claude",
		});
	});

	test("reads and writes direct publish mode in local Git config", async () => {
		const repository = await createTestRepository();

		expect(await readPublishMode(repository.worktreePath)).toBeNull();
		await writePublishMode(repository.worktreePath, "direct");

		expect(
			await runGit(["config", "--local", "--get", "how.publishMode"], {
				cwd: repository.worktreePath,
			}),
		).toBe("direct");
		expect(await readPublishMode(repository.worktreePath)).toBe("direct");
	});

	test("publishes current branch to origin and sets upstream", async () => {
		const repository = await createTestRepository();
		const remotePath = await createBareRepository();
		await createRegularCommit(repository);
		await runGit(["remote", "add", "origin", remotePath], { cwd: repository.worktreePath });
		const branchName = await runGit(["branch", "--show-current"], { cwd: repository.worktreePath });

		expect(await publishDirect(repository.worktreePath)).toEqual({ type: "published" });

		expect(
			await runGit(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"], {
				cwd: repository.worktreePath,
			}),
		).toBe(`origin/${branchName}`);
		expect(await runGit(["rev-parse", branchName], { cwd: remotePath })).toBeTruthy();
	});

	test("asks for a project destination when no remote exists", async () => {
		const repository = await createTestRepository();
		await createRegularCommit(repository);

		expect(await publishDirect(repository.worktreePath)).toEqual({ type: "needsDestination" });
	});

	test("adds origin from a project destination URL before publishing", async () => {
		const repository = await createTestRepository();
		const remotePath = await createBareRepository();
		await createRegularCommit(repository);
		const branchName = await runGit(["branch", "--show-current"], { cwd: repository.worktreePath });

		expect(await publishDirect(repository.worktreePath, { destinationUrl: remotePath })).toEqual({
			type: "published",
		});

		expect(await runGit(["remote", "get-url", "origin"], { cwd: repository.worktreePath })).toBe(
			remotePath,
		);
		expect(
			await runGit(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"], {
				cwd: repository.worktreePath,
			}),
		).toBe(`origin/${branchName}`);
	});

	test("builds checkpoint messages after staging changes", async () => {
		const repository = await createTestRepository();
		await fs.writeFile(path.join(repository.worktreePath, "readme.md"), "hello\n");

		await createCheckpointCommit(
			repository.id,
			async () =>
				await checkpointMessageForStagedChanges({
					agent: "none",
					date: new Date("2026-06-12T12:34:00Z"),
					logger: console,
					projectId: repository.id,
					worktreePath: repository.worktreePath,
				}),
		);

		expect(await runGit(["log", "-1", "--format=%s"], { cwd: repository.worktreePath })).toMatch(
			/^Checkpoint: /,
		);
	});
});
