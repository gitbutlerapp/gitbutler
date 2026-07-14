import { forgeReview, githubForgeInfo, mergeStatus, mockForge, repoInfo } from "../src/forge.ts";
import { applyUpstream, getButlerPort, openWorkspace, type GitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { waitForTestId } from "../src/util.ts";
import { expect, type Page } from "@playwright/test";
import type { FakeGitHubOptions, FakeGitHubServer } from "../src/fakeGithub.ts";

const PR_NUMBER = 42;

test("managed branch shows the PR derived from the backend forge cache", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const branch = "branch1";
	const server = await setupManagedProject(page, gitbutler, fakeGithub, {
		sourceBranch: branch,
	});
	await applyUpstream(gitbutler, branch);
	await mockReviewDetails(page, branch);

	await openWorkspace(page);
	await expect(await waitForTestId(page, "pr-review-badge")).toContainText(`PR #${PR_NUMBER}`);

	// Keep the server live through the assertion; fixture cleanup closes it.
	expect(server.repositoryUrl).toContain("127.0.0.1");
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

	test("metadata-less branch shows the cache-derived PR", async ({
		page,
		gitbutler,
		fakeGithub,
	}) => {
		const branch = "single-branch-fixture";
		const server = await fakeGithub({
			headRepoPath: gitbutler.pathInWorkdir("remote-project"),
			sourceBranch: branch,
			isFork: false,
		});
		await gitbutler.runScript("project-in-single-branch-mode.sh", [server.repositoryUrl]);
		await storeFakeGitHubEnterprisePat(page, server);
		await mockReviewDetails(page, branch);

		await openWorkspace(page);
		await expect(page.getByTestId("chrome-header-current-branch")).toContainText(branch);
		await expect(await waitForTestId(page, "pr-review-badge")).toContainText(`PR #${PR_NUMBER}`);
	});
});

test("a live empty review list clears a stale workspace badge", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const branch = "branch1";
	const server = await setupManagedProject(page, gitbutler, fakeGithub, {
		sourceBranch: branch,
	});
	await applyUpstream(gitbutler, branch);
	await mockReviewDetails(page, branch);
	await openWorkspace(page);
	await expect(await waitForTestId(page, "pr-review-badge")).toContainText(`PR #${PR_NUMBER}`);

	await gitbutler.runScript("age-forge-review-cache.sh", ["local-clone", `${PR_NUMBER}`]);
	server.setListed(false);
	const response = await page.request.post(`http://localhost:${getButlerPort()}/list_reviews`, {
		data: { projectId: projectIdFromPage(page), cacheConfig: "noCache" },
	});
	expect(response.ok()).toBe(true);

	await page.reload();
	await waitForTestId(page, "workspace-view");
	await expect(page.getByTestId("pr-review-badge")).toHaveCount(0);
	await expect(page.getByTestId("create-review-button")).toBeVisible();
});

test("creating a PR shows it from the optimistic cache insert before list propagation", async ({
	page,
	gitbutler,
	fakeGithub,
}) => {
	const branch = "branch1";
	await setupManagedProject(page, gitbutler, fakeGithub, {
		sourceBranch: branch,
		listed: false,
	});
	await applyUpstream(gitbutler, branch);
	await mockReviewDetails(page, branch);
	await openWorkspace(page);

	await expect(page.getByTestId("pr-review-badge")).toHaveCount(0);
	await page.getByTestId("create-review-button").click();
	await waitForTestId(page, "create-review-box");
	await page.getByTestId("create-review-box-create-button").click();

	await expect(await waitForTestId(page, "pr-review-badge")).toContainText(`PR #${PR_NUMBER}`);
});

async function setupManagedProject(
	page: Page,
	gitbutler: GitButler,
	fakeGithub: (options: FakeGitHubOptions) => Promise<FakeGitHubServer>,
	options: Pick<FakeGitHubOptions, "sourceBranch" | "listed">,
): Promise<FakeGitHubServer> {
	const server = await fakeGithub({
		headRepoPath: gitbutler.pathInWorkdir("remote-project"),
		isFork: false,
		...options,
	});
	await gitbutler.runScript("project-with-remote-branches.sh", [server.repositoryUrl]);
	await storeFakeGitHubEnterprisePat(page, server);
	return server;
}

async function storeFakeGitHubEnterprisePat(page: Page, server: FakeGitHubServer) {
	const response = await page.request.post(
		`http://localhost:${getButlerPort()}/store_github_enterprise_pat`,
		{
			data: { host: server.apiBaseUrl, accessToken: "fake-token" },
		},
	);
	expect(response.ok()).toBe(true);
	expect((await response.json()).type).toBe("success");
}

async function mockReviewDetails(page: Page, branch: string) {
	const review = forgeReview(PR_NUMBER, branch);
	await mockForge(page, {
		forge_info: githubForgeInfo(),
		get_review: review,
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: [],
	});
}

function projectIdFromPage(page: Page): string {
	const segments = new URL(page.url()).pathname.split("/").filter(Boolean);
	const projectId = segments[0];
	if (!projectId) throw new Error(`No project id in workspace URL: ${page.url()}`);
	return projectId;
}
