import {
	ciCheck,
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
import { expect, type Page } from "@playwright/test";
import type { ForgeInfo, ForgeReview } from "@gitbutler/but-sdk";

const PR_NUMBER = 42;
const BRANCH = "branch1";

/**
 * Put the workspace into a state where `branch1` has an associated
 * review (number 42). The whole forge surface is mocked, so no live
 * forge is involved; the stack/branch itself is real (created via
 * but-server). PR association flows:
 * list_reviews → PrNumberSync persists the number → get_review.
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
