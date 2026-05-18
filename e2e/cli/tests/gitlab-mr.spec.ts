import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { createTestRepo, type TestRepo } from "../src/repo.ts";
import { gitlab } from "../src/forge-api.ts";
import { GITLAB_TEST_TOKEN, GITLAB_TEST_REPO } from "../src/env.ts";

const CLONE_URL = `https://gitlab.com/${GITLAB_TEST_REPO}.git`;

describe("GitLab MR lifecycle", () => {
	let repo: TestRepo;
	const createdBranches: string[] = [];

	beforeAll(() => {
		if (!GITLAB_TEST_TOKEN || !GITLAB_TEST_REPO) {
			throw new Error(
				"GITLAB_TEST_TOKEN and GITLAB_TEST_REPO must be set to run GitLab e2e tests",
			);
		}
	});

	afterAll(async () => {
		for (const branch of createdBranches) {
			const mrs = await gitlab
				.listMergeRequests(GITLAB_TEST_REPO, {
					sourceBranch: branch,
					state: "opened",
				})
				.catch(() => []);
			for (const mr of mrs) {
				await gitlab.closeMergeRequest(GITLAB_TEST_REPO, mr.iid).catch(() => {});
			}
			await gitlab.deleteBranch(GITLAB_TEST_REPO, branch).catch(() => {});
		}

		await repo?.cleanup();
	});

	it("should create a single MR", async () => {
		repo = await createTestRepo({
			cloneUrl: CLONE_URL,
			token: GITLAB_TEST_TOKEN,
			forge: "gitlab",
			ownerRepo: GITLAB_TEST_REPO,
		});

		const branchName = `${repo.prefix}-single`;
		createdBranches.push(branchName);

		await repo.but("branch", "new", branchName);
		await repo.createFile("test-file.txt", `Created by e2e test ${repo.prefix}\n`);
		await repo.but("commit", branchName, "-m", "test: add test file");
		await repo.but("push", branchName);

		// `but mr` is an alias for `but pr` — use it to verify the alias works.
		await repo.but("mr", "new", branchName, "-t", "--no-hooks");

		const mrs = await gitlab.listMergeRequests(GITLAB_TEST_REPO, {
			sourceBranch: branchName,
		});
		expect(mrs.length).toBe(1);
		expect(mrs[0].state).toBe("opened");
	});

	it("should create a draft MR", async () => {
		const branchName = `${repo.prefix}-draft`;
		createdBranches.push(branchName);

		await repo.but("branch", "new", branchName);
		await repo.createFile("draft-file.txt", `Draft MR test ${repo.prefix}\n`);
		await repo.but("commit", branchName, "-m", "test: draft mr file");
		await repo.but("push", branchName);

		await repo.but("mr", "new", branchName, "-t", "-d", "--no-hooks");

		const mrs = await gitlab.listMergeRequests(GITLAB_TEST_REPO, {
			sourceBranch: branchName,
		});
		expect(mrs.length).toBe(1);
		// GitLab draft MRs have "Draft:" prefix in title or draft field.
		expect(mrs[0].draft).toBe(true);
	});

	it("should toggle draft state", async () => {
		const branchName = `${repo.prefix}-draft`;

		await repo.but("mr", "set-ready", branchName);

		let mrs = await gitlab.listMergeRequests(GITLAB_TEST_REPO, {
			sourceBranch: branchName,
		});
		expect(mrs[0].draft).toBe(false);

		await repo.but("mr", "set-draft", branchName);

		mrs = await gitlab.listMergeRequests(GITLAB_TEST_REPO, {
			sourceBranch: branchName,
		});
		expect(mrs[0].draft).toBe(true);
	});

	it("should create stacked MRs with correct targets", async () => {
		const base = `${repo.prefix}-stack-base`;
		const top = `${repo.prefix}-stack-top`;
		createdBranches.push(base, top);

		await repo.but("branch", "new", base);
		await repo.createFile("stack-base.txt", `Stack base ${repo.prefix}\n`);
		await repo.but("commit", base, "-m", "test: stack base commit");
		await repo.but("push", base);
		await repo.but("mr", "new", base, "-t", "--no-hooks");

		await repo.but("branch", "new", top);
		await repo.createFile("stack-top.txt", `Stack top ${repo.prefix}\n`);
		await repo.but("commit", top, "-m", "test: stack top commit");
		await repo.but("push", top);
		await repo.but("mr", "new", top, "-t", "--no-hooks");

		const topMrs = await gitlab.listMergeRequests(GITLAB_TEST_REPO, {
			sourceBranch: top,
		});
		expect(topMrs.length).toBe(1);
		expect(topMrs[0].target_branch).toBe(base);

		const baseMrs = await gitlab.listMergeRequests(GITLAB_TEST_REPO, {
			sourceBranch: base,
		});
		expect(baseMrs.length).toBe(1);
	});

	it("should update an MR after force-pushing", async () => {
		const branchName = `${repo.prefix}-amend`;
		createdBranches.push(branchName);

		await repo.but("branch", "new", branchName);
		await repo.createFile("amend-file.txt", `Original content ${repo.prefix}\n`);
		await repo.but("commit", branchName, "-m", "test: original commit");
		await repo.but("push", branchName);
		await repo.but("mr", "new", branchName, "-t", "--no-hooks");

		await repo.createFile("amend-file.txt", `Amended content ${repo.prefix}\n`);
		await repo.but("commit", branchName, "-m", "test: amended commit");
		await repo.but("push", branchName, "--with-force");

		const mrs = await gitlab.listMergeRequests(GITLAB_TEST_REPO, {
			sourceBranch: branchName,
		});
		expect(mrs.length).toBe(1);
		expect(mrs[0].state).toBe("opened");
	});
});
