import { expect } from "../src/expect.ts";
import {
	ciCheck,
	ciCheckRunning,
	forgeErrorBody,
	forgeReview,
	githubForgeInfo,
	gitlabForgeInfo,
	mergeStatus,
	mockForge,
	repoInfo,
	type ForgeMocks,
} from "../src/forge.ts";
import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { waitForTestId } from "../src/util.ts";
import { type Page, type Route } from "@playwright/test";
import type { ForgeInfo, ForgeReview } from "@gitbutler/but-sdk";

const PR_NUMBER = 42;
const BRANCH = "branch1";

/**
 * Put the workspace into a state where `branch1` has an associated
 * review (number 42). The whole forge surface is mocked, so no live
 * forge is involved; the stack/branch itself is real (created via
 * but-server). PR association flows:
 * The backend forge cache provides the branch association; browser routes still
 * mock review details/checks after the badge asks for the review number.
 *
 * `forgeInfo` selects which forge the renderer thinks it's talking to,
 * which drives capability gating and label text.
 */
async function openWorkspaceWithMockedPr(
	page: Page,
	gitbutler: { runScript: (s: string, a?: string[]) => Promise<void> },
	opts: {
		forgeInfo: ForgeInfo;
		checks?: NonNullable<ForgeMocks["list_ci_checks"]>;
	},
) {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler as never, BRANCH);

	const review = forgeReview(PR_NUMBER, BRANCH);
	await cacheReview(gitbutler, review);
	await mockForge(page, {
		forge_info: opts.forgeInfo,
		list_reviews: [review],
		get_review: review,
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
		list_ci_checks: opts.checks ?? [],
	});

	await openWorkspace(page);
	await waitForTestId(page, "branch-card");
}

test("CI badge shows passed when all checks succeed", async ({ page, gitbutler }) => {
	await openWorkspaceWithMockedPr(page, gitbutler, {
		forgeInfo: githubForgeInfo(),
		checks: [ciCheck("build", "success"), ciCheck("test", "success")],
	});

	const badge = await waitForTestId(page, "pr-checks-badge");
	await expect(badge).toContainText("Passed");
});

test("CI badge shows failed when a check fails", async ({ page, gitbutler }) => {
	await openWorkspaceWithMockedPr(page, gitbutler, {
		forgeInfo: githubForgeInfo(),
		checks: [ciCheck("build", "success"), ciCheck("test", "failure")],
	});

	const badge = await waitForTestId(page, "pr-checks-badge");
	await expect(badge).toContainText("Failed");
});

test("CI badge does not error for a PR from a fork", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler as never, BRANCH);

	const review = forgeReview(PR_NUMBER, BRANCH, {
		repositoryHttpsUrl: "https://github.com/contributor/widgets.git",
		repoOwner: "contributor",
		headRepoIsFork: true,
	});
	await cacheReview(gitbutler, review);
	await mockForge(page, {
		forge_info: githubForgeInfo(),
		list_reviews: [review],
		get_review: review,
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo({ fork: false }),
	});
	let ciChecksRequests = 0;
	await page.route("**/list_ci_checks", async (route) => {
		ciChecksRequests += 1;
		await route.fulfill({
			status: 200,
			contentType: "application/json",
			body: JSON.stringify({
				type: "error",
				subject: { message: "Failed to list checks for ref: 422" },
			}),
		});
	});

	await openWorkspace(page);
	await waitForTestId(page, "branch-card");

	const badge = await waitForTestId(page, "pr-checks-badge");
	await expect(badge).toContainText("No checks");
	await expect(badge).not.toContainText("Error");
	expect(ciChecksRequests).toBe(0);
});

/**
 * Open the workspace with a GitHub PR whose `list_ci_checks` command is routed
 * by `handler`, so a test can drive check-run failures / recovery / cadence
 * directly. The rest of the forge surface is mocked normally.
 */
async function openGithubPrWithChecksRoute(
	page: Page,
	gitbutler: { runScript: (s: string, a?: string[]) => Promise<void> },
	handler: (route: Route) => Promise<void> | void,
	reviewOverrides: Partial<ForgeReview> = {},
) {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler as never, BRANCH);

	const review = forgeReview(PR_NUMBER, BRANCH, reviewOverrides);
	await cacheReview(gitbutler, review);
	await mockForge(page, {
		forge_info: githubForgeInfo(),
		list_reviews: [review],
		get_review: review,
		get_review_merge_status: mergeStatus(),
		get_repo_info: repoInfo(),
	});
	await page.route("**/list_ci_checks", handler);

	await openWorkspace(page);
	await waitForTestId(page, "branch-card");
}

test("a failing checks poll backs off instead of hammering the endpoint", async ({
	page,
	gitbutler,
}) => {
	// A recent `modifiedAt` keeps the healthy schedule on its fast 5s interval,
	// so the back-off to 30s shows up as a drop in request cadence. An old date
	// would already sit on the slow schedule and hide the difference.
	const now = new Date().toISOString();
	let checkRequests = 0;
	await openGithubPrWithChecksRoute(
		page,
		gitbutler,
		async (route) => {
			checkRequests += 1;
			await route.fulfill({
				status: 200,
				contentType: "application/json",
				body: forgeErrorBody({ code: "NetworkError" }),
			});
		},
		{ modifiedAt: now, createdAt: now, lastSyncAt: now },
	);

	const badge = await waitForTestId(page, "pr-checks-badge");
	// The first failed poll surfaces the error state and trips the back-off.
	await expect(badge).toContainText("Error");

	const afterFirstError = checkRequests;
	// Longer than the fast 5s interval, shorter than the 30s back-off: a
	// hammering poller would fire ~2 more times in this window, a backed-off
	// one ~0.
	await page.waitForTimeout(13_000);
	expect(checkRequests - afterFirstError).toBeLessThanOrEqual(1);
});

test("retrying a failed checks badge recovers once the fetch succeeds", async ({
	page,
	gitbutler,
}) => {
	// Fail until the badge is clicked, then serve a running check. Clicking the
	// badge forces an immediate refetch, so recovery is deterministic instead of
	// waiting out the back-off interval.
	let shouldError = true;
	await openGithubPrWithChecksRoute(page, gitbutler, async (route) => {
		const body = shouldError
			? forgeErrorBody({ code: "NetworkError" })
			: JSON.stringify({ type: "success", subject: [ciCheckRunning("build")] });
		await route.fulfill({ status: 200, contentType: "application/json", body });
	});

	const badge = await waitForTestId(page, "pr-checks-badge");
	await expect(badge).toContainText("Error");

	shouldError = false;
	await badge.click();
	await expect(badge).toContainText("Running");
});

test("empty checks render as no-checks, not an error", async ({ page, gitbutler }) => {
	// A transient GitHub 422 (unresolvable ref) is mapped to an empty list by
	// the backend, so from the renderer it is just a successful empty result —
	// it must read as "No checks", never as an error.
	await openWorkspaceWithMockedPr(page, gitbutler, {
		forgeInfo: githubForgeInfo(),
		checks: [],
	});

	const badge = await waitForTestId(page, "pr-checks-badge");
	await expect(badge).toContainText("No checks");
	await expect(badge).not.toContainText("Error");
});

test("GitLab MR shows the MR review badge", async ({ page, gitbutler }) => {
	await openWorkspaceWithMockedPr(page, gitbutler, { forgeInfo: gitlabForgeInfo() });

	// The review badge labels the unit per forge: "MR !42" for GitLab
	// (vs "PR #42" for GitHub). Association runs because GitLab has the
	// listService capability.
	const badge = await waitForTestId(page, "pr-review-badge");
	await expect(badge).toContainText("MR");
	await expect(badge).toContainText("!42");
});

test("GitLab has no CI checks badge even when checks would resolve", async ({
	page,
	gitbutler,
}) => {
	// `gitlabForgeInfo` reports `capabilities.checks: false`. Even though
	// we mock a passing check, the badge must not render — the renderer
	// gates purely on the capability, never on forge name.
	await openWorkspaceWithMockedPr(page, gitbutler, {
		forgeInfo: gitlabForgeInfo(),
		checks: [ciCheck("build", "success")],
	});

	// The review badge confirms the MR card actually rendered, so the
	// checks-badge absence is a real capability gate, not just "no PR".
	await waitForTestId(page, "pr-review-badge");
	await expect(page.getByTestId("pr-checks-badge")).toHaveCount(0);
});

// Bitbucket and Azure have every forge capability off (no prService /
// listService / repoInfo / checks). The renderer should surface no
// review affordances at all — no review badge, no checks badge, no
// create-review button — regardless of what reviews the (mocked) forge
// would otherwise return.
for (const forge of ["bitbucket", "azure"] as const) {
	test(`${forge} surfaces no forge review affordances`, async ({ page, gitbutler }) => {
		await gitbutler.runScript("project-with-remote-branches.sh");
		await applyUpstream(gitbutler as never, BRANCH);

		const review = forgeReview(PR_NUMBER, BRANCH);
		await mockForge(page, {
			forge_info: { ...githubForgeInfo(), name: forge, capabilities: noCapabilities() },
			list_reviews: [review],
			get_review: review,
		});

		await openWorkspace(page);
		await waitForTestId(page, "branch-card");

		// No listService → no PR association → no review badge; and no
		// checks/create affordances either.
		await expect(page.getByTestId("pr-review-badge")).toHaveCount(0);
		await expect(page.getByTestId("pr-checks-badge")).toHaveCount(0);
		await expect(page.getByTestId("create-review-button")).toHaveCount(0);
	});
}

function noCapabilities() {
	return { checks: false, repoInfo: false, prService: false, listService: false };
}

// The merge button lives in the branch-view drawer (BranchReview →
// StackedPullRequestCard → PullRequestCard → MergeButton). Open it for
// the branch that owns the review, with a custom forge mock set.
async function openReviewBranchView(
	page: Page,
	gitbutler: { runScript: (s: string, a?: string[]) => Promise<void> },
	mocks: ForgeMocks,
	reviewOverrides: Partial<ForgeReview> = {},
) {
	// `sourceBranch` must stay `BRANCH` so the review associates with the
	// applied branch; callers override other fields (e.g. targetBranch).
	const review = forgeReview(PR_NUMBER, BRANCH, reviewOverrides);
	// Permissive repo info by default (canMerge=true) so the push-permission
	// gate passes and the merge button hinges on isMergeable / base-target.
	await mockForge(page, {
		list_reviews: [review],
		get_review: review,
		get_repo_info: repoInfo(),
		...mocks,
	});

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler as never, BRANCH);
	await cacheReview(gitbutler, review);
	await openWorkspace(page);

	await page.getByTestId("branch-header").filter({ hasText: BRANCH }).first().click();
	await waitForTestId(page, "branch-view");
	return await waitForTestId(page, "pr-merge-button");
}

// GitLab is the cleanest vehicle for the `isMergeable` gate: a permissive
// `get_repo_info` (canMerge=true, from `openReviewBranchView`) clears the
// push-permission gate, and its branch-name-only base check passes for an
// MR targeting `master`, so `isMergeable` is the deciding factor.
test("merge button is enabled when the MR is mergeable", async ({ page, gitbutler }) => {
	const mergeButton = await openReviewBranchView(page, gitbutler, {
		forge_info: gitlabForgeInfo(),
		get_review_merge_status: mergeStatus({ isMergeable: true }),
		get_review_base_repo_url: null,
	});
	await expect(mergeButton).toBeEnabled();
});

test("merge button is disabled when the MR is not mergeable", async ({ page, gitbutler }) => {
	// `mergeableState` is left out of the named-bad set (blocked/unknown/
	// behind/dirty) so the only gate that trips is `isMergeable: false`.
	const mergeButton = await openReviewBranchView(page, gitbutler, {
		forge_info: gitlabForgeInfo(),
		get_review_merge_status: mergeStatus({ isMergeable: false, mergeableState: "checking" }),
		get_review_base_repo_url: null,
	});
	await expect(mergeButton).toBeDisabled();
});

test("a failing merge-status poll backs off instead of hammering the endpoint", async ({
	page,
	gitbutler,
}) => {
	// A recent `modifiedAt` keeps the healthy schedule on its fast 5s interval,
	// so the back-off to 30s shows up as a drop in request cadence (see the
	// equivalent checks-poll test above). Routed before `openReviewBranchView`
	// so even the first status request fails.
	const now = new Date().toISOString();
	let statusRequests = 0;
	await page.route("**/get_review_merge_status", async (route) => {
		statusRequests += 1;
		await route.fulfill({
			status: 200,
			contentType: "application/json",
			body: forgeErrorBody({ code: "Unknown", message: "Failed to fetch PR merge status" }),
		});
	});

	const mergeButton = await openReviewBranchView(
		page,
		gitbutler,
		{ forge_info: gitlabForgeInfo(), get_review_base_repo_url: null },
		{ modifiedAt: now, createdAt: now, lastSyncAt: now },
	);
	// Status never loaded: merge stays unavailable, but as "not loaded",
	// never as a definitive "not mergeable".
	await expect(mergeButton).toBeDisabled();
	await mergeButton.hover();
	await expect(page.locator(".tooltip-container")).toContainText("merge status not loaded");
	await expect.poll(() => statusRequests).toBeGreaterThan(0);

	const afterFirstError = statusRequests;
	// Longer than the fast 5s interval, shorter than the 30s back-off: a
	// hammering poller would fire ~2 more times in this window, a backed-off
	// one ~0.
	await page.waitForTimeout(13_000);
	expect(statusRequests - afterFirstError).toBeLessThanOrEqual(1);
});

async function cacheReview(
	gitbutler: { runScript: (s: string, a?: string[]) => Promise<void> },
	review: ForgeReview,
) {
	await gitbutler.runScript("cache-forge-review.sh", [
		"local-clone",
		`${review.number}`,
		review.sourceBranch,
		review.targetBranch,
	]);
}

test("merge button is disabled when the PR targets a stacked branch, not the base", async ({
	page,
	gitbutler,
}) => {
	// In a stack only the bottom PR targets the base branch; PRs higher up
	// target the branch below them and aren't mergeable yet ("not next in
	// stack"). Reproduce that condition with a review whose targetBranch
	// is not the project base ("master") — the merge button must stay
	// disabled even though the forge reports it mergeable.
	const mergeButton = await openReviewBranchView(
		page,
		gitbutler,
		{
			forge_info: gitlabForgeInfo(),
			get_review_merge_status: mergeStatus({ isMergeable: true }),
			get_review_base_repo_url: null,
		},
		{ targetBranch: "branch-below-in-stack" },
	);
	await expect(mergeButton).toBeDisabled();
});

test("merge button is disabled when the PR's base is a different repo", async ({
	page,
	gitbutler,
}) => {
	// The base-branch comparison hashes the repo the workspace
	// *integrates from* (its upstream), not the fork it pushes to. The
	// ordinary contribution (head on a fork, base on upstream) therefore
	// merges fine — base == upstream == our base repo.
	//
	// This guards the inverse: a PR whose *base* is some other repo (e.g.
	// it targets your fork's `master` instead of the upstream you track).
	// The branch name still reads "master", so the name check passes, but
	// merging it wouldn't advance the branch this workspace integrates
	// from — and the button's post-merge base refresh would be wrong — so
	// it stays disabled ("not next in stack") even when the forge reports
	// it mergeable.
	//
	// Exercises the getBaseRepoUrl → pullRequestTargets →
	// baseIsTargetBranch wiring (GitHub path) that regressed once during
	// the forge refactor; the pure logic is unit-tested, this guards the
	// integration.
	const mergeButton = await openReviewBranchView(page, gitbutler, {
		forge_info: githubForgeInfo(),
		get_review_merge_status: mergeStatus({ isMergeable: true }),
		get_review_base_repo_url: "https://github.com/a-stranger/their-fork.git",
	});
	await expect(mergeButton).toBeDisabled();
});
