import { pullRequestTargetsBaseBranch } from "$lib/forge/shared/pullRequestTargets";
import { parseRemoteUrl } from "$lib/git/gitUrl";
import { describe, expect, test } from "vitest";
import type { PullRequest } from "$lib/forge/interface/types";

function makePr(overrides: Partial<PullRequest> = {}): PullRequest {
	return {
		htmlUrl: "https://github.com/upstream/proj/pull/1",
		number: 1,
		title: "x",
		body: undefined,
		author: null,
		labels: [],
		draft: false,
		sourceBranch: "feature",
		targetBranch: "main",
		sha: "deadbeef",
		createdAt: "2026-06-05T00:00:00Z",
		modifiedAt: "2026-06-05T00:00:00Z",
		mergedAt: undefined,
		closedAt: undefined,
		repositorySshUrl: undefined,
		repositoryHttpsUrl: undefined,
		repoOwner: undefined,
		reviewers: [],
		headRepoIsFork: false,
		...overrides,
	};
}

const UPSTREAM_URL = "https://github.com/upstream/proj.git";
const FORK_URL = "https://github.com/contributor/proj.git";
const UPSTREAM_HASH = parseRemoteUrl(UPSTREAM_URL)?.hash;

describe("pullRequestTargetsBaseBranch", () => {
	test("returns false when there is no PR", () => {
		expect(
			pullRequestTargetsBaseBranch({
				pr: undefined,
				baseBranchShortName: "main",
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: UPSTREAM_URL,
				forgeName: "github",
			}),
		).toBe(false);
	});

	test("returns false when the PR targets a different branch name", () => {
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "develop" }),
				baseBranchShortName: "main",
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: UPSTREAM_URL,
				forgeName: "github",
			}),
		).toBe(false);
	});

	test("returns true on GitHub when branch and base-repo hash both match", () => {
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "main" }),
				baseBranchShortName: "main",
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: UPSTREAM_URL,
				forgeName: "github",
			}),
		).toBe(true);
	});

	test("returns false on GitHub when the PR targets a *fork* — branch matches but base repo differs (regression)", () => {
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "main" }),
				baseBranchShortName: "main",
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: FORK_URL,
				forgeName: "github",
			}),
		).toBe(false);
	});

	test("returns false on GitHub when the PR's base repo URL is missing — safe default, don't enable merge for an unknown target", () => {
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "main" }),
				baseBranchShortName: "main",
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: null,
				forgeName: "github",
			}),
		).toBe(false);
	});

	test("returns false on GitHub when the project hasn't reported its base repo hash", () => {
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "main" }),
				baseBranchShortName: "main",
				baseBranchRepoHash: undefined,
				prBaseRepoUrl: UPSTREAM_URL,
				forgeName: "github",
			}),
		).toBe(false);
	});

	test("GitLab: branch-name match alone is sufficient — fork model uses project IDs, not URL hashes", () => {
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "main" }),
				baseBranchShortName: "main",
				// Even if base repo hash and URL would otherwise mismatch:
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: FORK_URL,
				forgeName: "gitlab",
			}),
		).toBe(true);
	});

	test("GitLab: branch-name mismatch still wins", () => {
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "develop" }),
				baseBranchShortName: "main",
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: UPSTREAM_URL,
				forgeName: "gitlab",
			}),
		).toBe(false);
	});

	test("Bitbucket: branch-name match alone is sufficient — backend exposes no base-repo URL", () => {
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "main" }),
				baseBranchShortName: "main",
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: null,
				forgeName: "bitbucket",
			}),
		).toBe(true);
	});

	test("Bitbucket: branch-name mismatch still wins", () => {
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "develop" }),
				baseBranchShortName: "main",
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: null,
				forgeName: "bitbucket",
			}),
		).toBe(false);
	});

	test("unknown forge name falls back to URL-hash comparison (treats like GitHub)", () => {
		// New forges should not silently match by branch name only — the
		// branch-only short-circuit is intentionally limited to GitLab and
		// Bitbucket. Any other value defaults to the strict GitHub-style
		// comparison.
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "main" }),
				baseBranchShortName: "main",
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: FORK_URL,
				forgeName: "azure",
			}),
		).toBe(false);
	});

	test("undefined forge name falls back to URL-hash comparison", () => {
		// Same as above — no forge metadata available shouldn't relax
		// the fork check.
		expect(
			pullRequestTargetsBaseBranch({
				pr: makePr({ targetBranch: "main" }),
				baseBranchShortName: "main",
				baseBranchRepoHash: UPSTREAM_HASH,
				prBaseRepoUrl: UPSTREAM_URL,
				forgeName: undefined,
			}),
		).toBe(true);
	});

	test("repo hash is invariant across https/ssh URL forms — verifies the parseRemoteUrl assumption the comparison rests on", () => {
		// If this ever breaks, the fork-detection above silently
		// starts treating ssh remotes as forks of their https selves.
		const https = parseRemoteUrl("https://github.com/upstream/proj.git")?.hash;
		const ssh = parseRemoteUrl("git@github.com:upstream/proj.git")?.hash;
		expect(https).toBeDefined();
		expect(ssh).toBe(https);
	});
});
