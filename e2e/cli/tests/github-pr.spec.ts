import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { createTestRepo, type TestRepo } from "../src/repo.ts";
import { github } from "../src/forge-api.ts";
import { GITHUB_TEST_TOKEN, GITHUB_TEST_REPO } from "../src/env.ts";

const CLONE_URL = `https://github.com/${GITHUB_TEST_REPO}.git`;

describe("GitHub PR lifecycle", () => {
	let repo: TestRepo;
	const createdBranches: string[] = [];

	beforeAll(() => {
		if (!GITHUB_TEST_TOKEN || !GITHUB_TEST_REPO) {
			throw new Error(
				"GITHUB_TEST_TOKEN and GITHUB_TEST_REPO must be set to run GitHub e2e tests",
			);
		}
	});

	afterAll(async () => {
		// Close any PRs we opened and delete remote branches.
		const [owner] = GITHUB_TEST_REPO.split("/");
		for (const branch of createdBranches) {
			const prs = await github.listPullRequests(GITHUB_TEST_REPO, {
				head: `${owner}:${branch}`,
				state: "open",
			});
			for (const pr of prs) {
				await github.closePullRequest(GITHUB_TEST_REPO, pr.number).catch(() => {});
			}
			await github.deleteRef(GITHUB_TEST_REPO, branch).catch(() => {});
		}

		await repo?.cleanup();
	});

	it("should create a single PR", async () => {
		repo = await createTestRepo({
			cloneUrl: CLONE_URL,
			token: GITHUB_TEST_TOKEN,
			forge: "github",
			ownerRepo: GITHUB_TEST_REPO,
		});

		const branchName = `${repo.prefix}-single`;
		createdBranches.push(branchName);

		// Create a branch and add a commit.
		await repo.but("branch", "new", branchName);
		await repo.createFile("test-file.txt", `Created by e2e test ${repo.prefix}\n`);
		await repo.but("commit", branchName, "-m", "test: add test file");

		// Push the branch.
		await repo.but("push", branchName);

		// Create a PR using the CLI.
		await repo.but(
			"pr",
			"new",
			branchName,
			"-t",
			"--no-hooks",
		);

		// Verify the PR exists via the GitHub API.
		const [owner] = GITHUB_TEST_REPO.split("/");
		const prs = await github.listPullRequests(GITHUB_TEST_REPO, {
			head: `${owner}:${branchName}`,
		});
		expect(prs.length).toBe(1);
		expect(prs[0].state).toBe("open");
	});

	it("should create a draft PR", async () => {
		const branchName = `${repo.prefix}-draft`;
		createdBranches.push(branchName);

		await repo.but("branch", "new", branchName);
		await repo.createFile("draft-file.txt", `Draft PR test ${repo.prefix}\n`);
		await repo.but("commit", branchName, "-m", "test: draft pr file");
		await repo.but("push", branchName);

		await repo.but("pr", "new", branchName, "-t", "-d", "--no-hooks");

		const [owner] = GITHUB_TEST_REPO.split("/");
		const prs = await github.listPullRequests(GITHUB_TEST_REPO, {
			head: `${owner}:${branchName}`,
		});
		expect(prs.length).toBe(1);
		expect(prs[0].draft).toBe(true);
	});

	it("should toggle draft state", async () => {
		// Reuse the draft PR from the previous test.
		const branchName = `${repo.prefix}-draft`;
		const [owner] = GITHUB_TEST_REPO.split("/");

		// Set it to ready.
		await repo.but("pr", "set-ready", branchName);

		let prs = await github.listPullRequests(GITHUB_TEST_REPO, {
			head: `${owner}:${branchName}`,
		});
		expect(prs[0].draft).toBe(false);

		// Set it back to draft.
		await repo.but("pr", "set-draft", branchName);

		prs = await github.listPullRequests(GITHUB_TEST_REPO, {
			head: `${owner}:${branchName}`,
		});
		expect(prs[0].draft).toBe(true);
	});

	it("should create stacked PRs with correct targets", async () => {
		const base = `${repo.prefix}-stack-base`;
		const top = `${repo.prefix}-stack-top`;
		createdBranches.push(base, top);

		// Create first branch in the stack.
		await repo.but("branch", "new", base);
		await repo.createFile("stack-base.txt", `Stack base ${repo.prefix}\n`);
		await repo.but("commit", base, "-m", "test: stack base commit");
		await repo.but("push", base);
		await repo.but("pr", "new", base, "-t", "--no-hooks");

		// Create second branch on top of the first.
		await repo.but("branch", "new", top);
		await repo.createFile("stack-top.txt", `Stack top ${repo.prefix}\n`);
		await repo.but("commit", top, "-m", "test: stack top commit");
		await repo.but("push", top);
		await repo.but("pr", "new", top, "-t", "--no-hooks");

		// Verify the top PR targets the base branch, not the default branch.
		const [owner] = GITHUB_TEST_REPO.split("/");
		const topPrs = await github.listPullRequests(GITHUB_TEST_REPO, {
			head: `${owner}:${top}`,
		});
		expect(topPrs.length).toBe(1);
		expect(topPrs[0].base.ref).toBe(base);

		const basePrs = await github.listPullRequests(GITHUB_TEST_REPO, {
			head: `${owner}:${base}`,
		});
		expect(basePrs.length).toBe(1);
		// The base PR should target the repo's default branch (usually main or master).
	});

	it("should update a PR after amending and force-pushing", async () => {
		const branchName = `${repo.prefix}-amend`;
		createdBranches.push(branchName);

		await repo.but("branch", "new", branchName);
		await repo.createFile("amend-file.txt", `Original content ${repo.prefix}\n`);
		await repo.but("commit", branchName, "-m", "test: original commit");
		await repo.but("push", branchName);
		await repo.but("pr", "new", branchName, "-t", "--no-hooks");

		// Amend the commit with new content.
		await repo.createFile("amend-file.txt", `Amended content ${repo.prefix}\n`);
		await repo.but("commit", branchName, "-m", "test: amended commit");
		await repo.but("push", branchName, "--with-force");

		// PR should still be open.
		const [owner] = GITHUB_TEST_REPO.split("/");
		const prs = await github.listPullRequests(GITHUB_TEST_REPO, {
			head: `${owner}:${branchName}`,
		});
		expect(prs.length).toBe(1);
		expect(prs[0].state).toBe("open");
	});
});
