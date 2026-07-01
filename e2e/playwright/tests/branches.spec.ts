import { assertBranch, createNewBranch, deleteBranch, unapplyStack } from "../src/branch.ts";
import { startFakeGitHubServer, type FakeGitHubServer } from "../src/fakeGithub.ts";
import { applyUpstream, getButlerPort, openWorkspace, type GitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	clickByTestId,
	commitRow,
	fillByTestId,
	getByTestId,
	stack,
	textEditorFillByTestId,
	waitForTestId,
	waitForTestIdToNotExist,
} from "../src/util.ts";
import { expect, type Page } from "@playwright/test";
import { execFileSync } from "child_process";
import { existsSync, readFileSync, writeFileSync } from "fs";

/**
 * Navigate to the branches page and assert the standard 3-card layout from
 * `project-with-remote-branches.sh`. Filters on the `origin/master` header
 * because workspace stack headers also use the `branch-header` test id.
 */
async function gotoBranchesView(page: Page) {
	await clickByTestId(page, "navigation-branches-button");
	const originHeader = getByTestId(page, "branch-header").filter({ hasText: "origin/master" });
	await expect(originHeader).toBeVisible();

	const cards = getByTestId(page, "branch-list-card");
	await expect(cards).toHaveCount(3);
}

/**
 * Navigate to the branches page and apply a branch from its card.
 */
async function applyBranchFromBranchesView(page: Page, branchName: string) {
	await gotoBranchesView(page);

	await getByTestId(page, "branch-list-card").filter({ hasText: branchName }).click();
	await waitForTestId(page, "branches-view-delete-local-branch-button");

	await clickByTestId(page, "branches-view-apply-branch-button");
	await waitForTestId(page, "workspace-view");
}

async function syncAndIntegrate(page: Page) {
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "upstream-commits-integrate-button");
	await waitForTestId(page, "branch-integration-apply-button");
	await clickByTestId(page, "branch-integration-apply-button");
	await waitForTestIdToNotExist(page, "branch-integration-modal");
}

async function storeFakeGitHubEnterprisePat(page: Page, fakeGitHub: FakeGitHubServer) {
	const response = await page.request.post(
		`http://localhost:${getButlerPort()}/store_github_enterprise_pat`,
		{
			data: {
				host: fakeGitHub.apiBaseUrl,
				accessToken: "fake-token",
			},
		},
	);
	expect(response.ok()).toBe(true);
	const body = await response.json();
	expect(body.type).toBe("success");
}

function git(pathToRepo: string, args: string[]): string {
	return execFileSync("git", args, {
		cwd: pathToRepo,
		encoding: "utf8",
	}).trim();
}

async function openProjectWithFakeGitHub(
	page: Page,
	gitbutler: GitButler,
	fakeGitHub: FakeGitHubServer,
) {
	await gitbutler.runScript("project-with-github-fork-pr.sh", [fakeGitHub.repositoryUrl]);
	await openWorkspace(page);
	await storeFakeGitHubEnterprisePat(page, fakeGitHub);
	await page.reload();
	await waitForTestId(page, "workspace-view");
}

async function applyReviewFromBranchesView(page: Page, title: string) {
	await clickByTestId(page, "navigation-branches-button");
	await expect(getByTestId(page, "pr-list-card").filter({ hasText: "#42" })).toBeVisible();
	await getByTestId(page, "pr-list-card").filter({ hasText: title }).click();
	await clickByTestId(page, "branches-view-apply-from-fork-button");
	await waitForTestId(page, "workspace-view");
}

async function applyReviewBranchFromBranchesView(page: Page, branchName: string) {
	await clickByTestId(page, "navigation-branches-button");
	await waitForTestId(page, "branches-view");
	await expect(getByTestId(page, "branch-list-card").filter({ hasText: "#42" })).toBeVisible();
	await getByTestId(page, "branch-list-card").filter({ hasText: branchName }).click();
	await waitForTestId(page, "branches-view-apply-branch-button");
	await clickByTestId(page, "branches-view-apply-branch-button");
	await waitForTestId(page, "workspace-view");
}

test("should be able to apply a remote branch", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await applyBranchFromBranchesView(page, "branch1");

	await expect(stack(page, "branch1")).toHaveCount(1);
	await expect(commitRow(page)).toHaveCount(2);
});

test.describe("GitHub review apply", () => {
	test.describe.configure({ mode: "serial" });

	test("should apply a GitHub fork PR by creating the fork remote in the backend", async ({
		page,
		gitbutler,
	}) => {
		const localClone = gitbutler.pathInWorkdir("local-clone");
		const forkRepoPath = gitbutler.pathInWorkdir("fork-project-bare");
		const fakeGitHub = await startFakeGitHubServer({ forkRepoPath });
		try {
			await openProjectWithFakeGitHub(page, gitbutler, fakeGitHub);
			await applyReviewFromBranchesView(page, "Fork PR");

			expect(git(localClone, ["remote", "get-url", "contributor-user"])).toBe(forkRepoPath);
			expect(
				git(localClone, ["show-ref", "--verify", "refs/remotes/contributor-user/fork-feature"]),
			).toContain("refs/remotes/contributor-user/fork-feature");
			expect(readFileSync(gitbutler.pathInWorkdir("local-clone", "fork-pr.txt"), "utf-8")).toBe(
				"fork pr content\n",
			);
			await expect(stack(page, "fork-feature")).toHaveCount(1);
			await expect(getByTestId(page, "pr-review-badge")).toContainText("PR");
			await expect(getByTestId(page, "pr-review-badge")).toContainText("#42");
		} finally {
			await fakeGitHub.close();
		}
	});

	test("should apply a GitHub fork PR by reusing an existing fork remote", async ({
		page,
		gitbutler,
	}) => {
		const localClone = gitbutler.pathInWorkdir("local-clone");
		const forkRepoPath = gitbutler.pathInWorkdir("fork-project-bare");
		const fakeGitHub = await startFakeGitHubServer({ forkRepoPath });
		try {
			await gitbutler.runScript("project-with-github-fork-pr.sh", [fakeGitHub.repositoryUrl]);
			git(localClone, ["remote", "add", "contributor-user", forkRepoPath]);
			await openWorkspace(page);
			await storeFakeGitHubEnterprisePat(page, fakeGitHub);
			await page.reload();
			await waitForTestId(page, "workspace-view");

			await applyReviewBranchFromBranchesView(page, "fork-feature");

			expect(git(localClone, ["remote", "get-url", "contributor-user"])).toBe(forkRepoPath);
			expect(git(localClone, ["remote"]).split("\n")).not.toContain("contributor-user-2");
			await expect(stack(page, "fork-feature")).toHaveCount(1);
		} finally {
			await fakeGitHub.close();
		}
	});

	test("should apply a GitHub PR from the base repository to the managed workspace", async ({
		page,
		gitbutler,
	}) => {
		const localClone = gitbutler.pathInWorkdir("local-clone");
		const baseRepoPath = gitbutler.pathInWorkdir("remote-project");
		const fakeGitHub = await startFakeGitHubServer({
			headRepoPath: baseRepoPath,
			sourceBranch: "base-pr-feature",
			repoOwner: "acme",
			title: "Base PR",
			isFork: false,
		});
		try {
			await openProjectWithFakeGitHub(page, gitbutler, fakeGitHub);
			await applyReviewBranchFromBranchesView(page, "base-pr-feature");

			expect(readFileSync(gitbutler.pathInWorkdir("local-clone", "base-pr.txt"), "utf-8")).toBe(
				"base pr content\n",
			);
			await assertBranch("gitbutler/workspace", localClone);
			await expect(stack(page, "base-pr-feature")).toHaveCount(1);
			await expect(getByTestId(page, "pr-review-badge")).toContainText("#42");
		} finally {
			await fakeGitHub.close();
		}
	});

	test.describe("single-branch mode", () => {
		test.use({
			gitbutlerOptions: {
				config: {
					onboardingComplete: true,
					featureFlags: { singleBranch: true },
				},
			},
		});

		test("should apply a GitHub PR from the base repository", async ({ page, gitbutler }) => {
			const localClone = gitbutler.pathInWorkdir("local-clone");
			const baseRepoPath = gitbutler.pathInWorkdir("remote-project");
			const fakeGitHub = await startFakeGitHubServer({
				headRepoPath: baseRepoPath,
				sourceBranch: "base-pr-feature",
				repoOwner: "acme",
				title: "Base PR",
				isFork: false,
			});
			try {
				await gitbutler.runScript("project-with-github-fork-pr.sh", [fakeGitHub.repositoryUrl]);
				git(localClone, [
					"checkout",
					"-b",
					"single-branch-fixture",
					"origin/single-branch-fixture",
				]);
				await openWorkspace(page);
				await expect(getByTestId(page, "chrome-header-current-branch")).toContainText(
					"single-branch-fixture",
				);
				await storeFakeGitHubEnterprisePat(page, fakeGitHub);
				await page.reload();
				await waitForTestId(page, "workspace-view");

				await applyReviewBranchFromBranchesView(page, "base-pr-feature");

				await assertBranch("gitbutler/workspace", localClone);
				expect(
					readFileSync(gitbutler.pathInWorkdir("local-clone", "single-branch.txt"), "utf-8"),
				).toBe("single branch content\n");
				expect(readFileSync(gitbutler.pathInWorkdir("local-clone", "base-pr.txt"), "utf-8")).toBe(
					"base pr content\n",
				);
				await expect(stack(page, "single-branch-fixture")).toHaveCount(1);
				await expect(stack(page, "base-pr-feature")).toHaveCount(1);
			} finally {
				await fakeGitHub.close();
			}
		});

		test("should apply a GitHub fork PR", async ({ page, gitbutler }) => {
			const localClone = gitbutler.pathInWorkdir("local-clone");
			const forkRepoPath = gitbutler.pathInWorkdir("fork-project-bare");
			const fakeGitHub = await startFakeGitHubServer({ forkRepoPath });
			try {
				await gitbutler.runScript("project-with-github-fork-pr.sh", [fakeGitHub.repositoryUrl]);
				git(localClone, [
					"checkout",
					"-b",
					"single-branch-fixture",
					"origin/single-branch-fixture",
				]);
				await openWorkspace(page);
				await expect(getByTestId(page, "chrome-header-current-branch")).toContainText(
					"single-branch-fixture",
				);
				await storeFakeGitHubEnterprisePat(page, fakeGitHub);
				await page.reload();
				await waitForTestId(page, "workspace-view");

				await applyReviewFromBranchesView(page, "Fork PR");

				await assertBranch("gitbutler/workspace", localClone);
				expect(git(localClone, ["remote", "get-url", "contributor-user"])).toBe(forkRepoPath);
				expect(
					readFileSync(gitbutler.pathInWorkdir("local-clone", "single-branch.txt"), "utf-8"),
				).toBe("single branch content\n");
				expect(readFileSync(gitbutler.pathInWorkdir("local-clone", "fork-pr.txt"), "utf-8")).toBe(
					"fork pr content\n",
				);
				await expect(stack(page, "single-branch-fixture")).toHaveCount(1);
				await expect(stack(page, "fork-feature")).toHaveCount(1);
			} finally {
				await fakeGitHub.close();
			}
		});
	});
});

test("should be able to apply a remote branch and integrate the remote changes - simple", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await applyBranchFromBranchesView(page, "branch1");
	await expect(commitRow(page)).toHaveCount(2);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-remote-branch.sh");
	await syncAndIntegrate(page);

	await expect(commitRow(page)).toHaveCount(3);
});

test("should preview branch integration for a diverged branch with a parallel empty branch", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-diverged-branch-and-parallel-empty-branch.sh");
	await openWorkspace(page);

	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "upstream-commits-integrate-button");

	await waitForTestId(page, "branch-integration-modal");
	await expect(getByTestId(page, "branch-integration-preview-row").first()).toBeVisible();
	await expect(getByTestId(page, "branch-integration-error")).toHaveCount(0);
});

test("should be able to apply a remote branch and integrate the remote changes - create commit", async ({
	page,
	gitbutler,
}) => {
	const fileCPath = gitbutler.pathInWorkdir("local-clone/c_file");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(stack(page, "branch1")).toHaveCount(1);
	await expect(commitRow(page)).toHaveCount(2);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-remote-branch.sh");
	await clickByTestId(page, "sync-button");

	// New local commit
	writeFileSync(fileCPath, "This is file C\n", { flag: "w" });
	await clickByTestId(page, "start-commit-button");
	await waitForTestId(page, "new-commit-view");

	const newCommitMessage = "New local commit: adding file C";
	await fillByTestId(page, "commit-drawer-title-input", newCommitMessage);
	await textEditorFillByTestId(page, "commit-drawer-description-input", "CCCCCCC");
	await clickByTestId(page, "commit-drawer-action-button");

	// Integrate upstream commits on top
	await clickByTestId(page, "upstream-commits-integrate-button");
	await waitForTestId(page, "branch-integration-apply-button");
	await clickByTestId(page, "branch-integration-apply-button");
	await waitForTestIdToNotExist(page, "branch-integration-modal");

	const commits = commitRow(page);
	await expect(commits).toHaveCount(4);
	await expect(commits.nth(0)).toContainText(newCommitMessage);
});

test("should be able to apply a remote branch and integrate the remote changes - conflict", async ({
	page,
	gitbutler,
}) => {
	const filePath = gitbutler.pathInWorkdir("local-clone/a_file");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(commitRow(page)).toHaveCount(2);

	writeFileSync(filePath, "conflicting change\n", { flag: "a" });

	await clickByTestId(page, "start-commit-button");
	await waitForTestId(page, "new-commit-view");

	const newCommitMessage = "Conflicting change commit";
	await fillByTestId(page, "commit-drawer-title-input", newCommitMessage);
	await textEditorFillByTestId(
		page,
		"commit-drawer-description-input",
		"This should be oh-so-bad 🤭",
	);
	await clickByTestId(page, "commit-drawer-action-button");

	await expect(commitRow(page)).toHaveCount(3);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-remote-branch.sh");
	await syncAndIntegrate(page);

	const commits = commitRow(page);
	await expect(commits).toHaveCount(4);

	const conflictedCommit = commitRow(page, newCommitMessage);
	await conflictedCommit.click();
	await clickByTestId(page, "commit-drawer-resolve-conflicts-button");
	await waitForTestId(page, "edit-mode");

	expect(readFileSync(filePath, "utf-8")).toEqual(
		`foo
bar
baz
branch1 commit 1
branch1 commit 2
<<<<<` +
			`<< New base: branch1: third commit
branch1 commit 3
||||||| Common ancestor
=======
conflicting change
>>>>>>> Current commit: Conflicting change commit
`,
	);

	const resolved = `foo
bar
baz
branch1 commit 1
branch1 commit 2
branch1 commit 3
conflicting change
`;
	writeFileSync(filePath, resolved, { flag: "w" });

	await clickByTestId(page, "edit-mode-save-and-exit-button");
	await waitForTestId(page, "workspace-view");

	await expect(commitRow(page)).toHaveCount(4);
	expect(readFileSync(filePath, "utf-8")).toEqual(resolved);
});

test("should be able gracefully handle adding a branch that is ahead of our target commit", async ({
	page,
	gitbutler,
}) => {
	const fileBPath = gitbutler.pathInWorkdir("local-clone/b_file");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-base-and-branch.sh");
	await clickByTestId(page, "sync-button");

	await applyBranchFromBranchesView(page, "branch1");

	// 3 commits from branch1 + 1 base commit
	await expect(commitRow(page)).toHaveCount(4);
	expect(existsSync(fileBPath)).toBe(true);
});

// TODO: The integrate-upstream-commits-button assertion fails because target.sha
// is now set correctly after upstream integration, so no further integration is detected.
test.skip("should be able gracefully handle adding a branch that is behind of our target commit", async ({
	page,
	gitbutler,
}) => {
	const filePath = gitbutler.pathInWorkdir("local-clone/a_file");
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-base.sh");
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");
	await waitForTestIdToNotExist(page, "integrate-upstream-commits-button");

	await applyBranchFromBranchesView(page, "branch1");
	await expect(getByTestId(page, "integrate-upstream-commits-button")).toBeVisible();

	await expect(commitRow(page)).toHaveCount(2);
	const conflictedCommit = commitRow(page, "branch1: first commit");
	await conflictedCommit.click();
	await expect(getByTestId(page, "commit-drawer-resolve-conflicts-button")).toHaveCount(0);
	expect(readFileSync(filePath, "utf-8")).toEqual(
		`foo
bar
baz
branch1 commit 1
branch1 commit 2
`,
	);
});

test("should handle gracefully applying two conflicting branches", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(commitRow(page)).toHaveCount(2);

	await gotoBranchesView(page);
	await getByTestId(page, "branch-list-card").filter({ hasText: "branch2" }).click();
	await clickByTestId(page, "branches-view-apply-branch-button");
	await waitForTestId(page, "workspace-view");

	await waitForTestId(page, "branch-apply-conflict-toast");
});

test("should update the stale selection of an unexisting branch", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await gotoBranchesView(page);
	await getByTestId(page, "branch-list-card").filter({ hasText: "branch1" }).click();

	await clickByTestId(page, "navigation-workspace-button");
	await waitForTestId(page, "workspace-view");
	await expect(stack(page)).toHaveCount(1);

	// branch1 was merged in the forge — sync and integrate it away.
	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");
	await waitForTestIdToNotExist(page, "integrate-upstream-action-button");

	await waitForTestIdToNotExist(page, "stack");

	await gotoBranchesView(page);

	// We don't prune, so 3 branches remain, but branch1 is not selected anymore.
	const cardsAfter = getByTestId(page, "branch-list-card");
	await expect(cardsAfter.filter({ hasText: "branch1" })).not.toHaveClass(/\bselected\b/);
	await expect(getByTestId(page, "current-origin-list-card")).toHaveClass(/\bselected\b/);
});

test("should be able to delete a local branch", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1", "branch3");
	await gitbutler.runScript("move-branch.sh", ["branch3", "branch1", "local-clone"]);
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-header")).toHaveCount(2);

	await deleteBranch(page, "branch1");
	await waitForTestId(page, "workspace-view");

	await expect(stack(page)).toHaveCount(1);
	const headers = getByTestId(page, "branch-header");
	await expect(headers).toHaveCount(1);
	await expect(headers.first()).toContainText("branch3");
});

test("should be able to delete an empty local branch", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await createNewBranch(page, "new-branch");
	await expect(stack(page)).toHaveCount(1);

	await deleteBranch(page, "new-branch");
	await waitForTestId(page, "workspace-view");

	await expect(stack(page)).toHaveCount(0);
	await expect(getByTestId(page, "branch-header")).toHaveCount(0);
});

test("should be able to unapply a stack", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-header").filter({ hasText: "branch1" })).toBeVisible();

	await unapplyStack(page, "branch1");
	await waitForTestId(page, "workspace-view");

	await expect(stack(page)).toHaveCount(0);
	await expect(getByTestId(page, "branch-header").filter({ hasText: "branch1" })).toHaveCount(0);
});

test("should be able to move a branch when origin/master has advanced past the fork point", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1", "branch3");

	// Advance origin/master past the fork point of branch1/branch3 so the old
	// fork point becomes an unnamed segment in the graph.
	await gitbutler.runScript("project-with-remote-branches__add-commit-to-base.sh");
	await gitbutler.runScript("fetch-in-clone.sh", ["local-clone"]);
	// Move branch3 on top of branch1 — must succeed even with a nameless base segment.
	await gitbutler.runScript("move-branch.sh", ["branch3", "branch1", "local-clone"]);

	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-header")).toHaveCount(2);
});
