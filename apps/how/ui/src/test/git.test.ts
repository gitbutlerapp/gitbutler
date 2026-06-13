import { createCheckpointCommit, listCheckpointCommits, runGit } from "../../../electron/src/git";
import { describe, expect, test } from "vitest";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

async function createTestRepository(): Promise<string> {
	const repositoryPath = await fs.mkdtemp(path.join(os.tmpdir(), "how-git-test-"));
	await runGit(["init"], { cwd: repositoryPath });
	await runGit(["config", "user.name", "How Test"], { cwd: repositoryPath });
	await runGit(["config", "user.email", "how-test@example.com"], { cwd: repositoryPath });
	return repositoryPath;
}

describe("git helpers", () => {
	test("lists checkpoint commits by their message prefix", async () => {
		const repositoryPath = await createTestRepository();

		await fs.writeFile(path.join(repositoryPath, "readme.md"), "hello\n");
		await createCheckpointCommit(repositoryPath, "Checkpoint: Jun 13, 09:30");

		await fs.writeFile(path.join(repositoryPath, "readme.md"), "hello again\n");
		await runGit(["add", "--all"], { cwd: repositoryPath });
		await runGit(["commit", "--no-gpg-sign", "--message", "Regular commit"], {
			cwd: repositoryPath,
		});

		const checkpoints = await listCheckpointCommits(repositoryPath, 10);

		expect(checkpoints).toHaveLength(1);
		expect(checkpoints[0]?.title).toBe("Checkpoint: Jun 13, 09:30");
	});
});
