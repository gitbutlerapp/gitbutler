import type {
	CiCheck,
	CiConclusion,
	ForgeInfo,
	ForgeReview,
	RepoInfo,
	ReviewMergeStatus,
} from "@gitbutler/but-sdk";
import type { Page, Route } from "@playwright/test";

/**
 * Mock forge (GitHub/GitLab/…) responses in Playwright e2e tests.
 *
 * Since the forge refactor, the desktop renderer no longer talks to
 * github.com / gitlab.com directly — every forge-aware decision flows
 * through a small set of backend commands. In the web/e2e build those
 * commands are plain `POST {butServer}/{command}` calls returning
 * `{ type: "success", subject }` (see `apps/desktop/src/lib/backend/web.ts`).
 *
 * So we can mock the whole forge surface by intercepting just those
 * command endpoints with `page.route()` and returning typed fixtures.
 * Everything else (the real workspace, stacks, commits) still runs
 * against the live but-server.
 *
 * Call `mockForge` BEFORE navigating (e.g. before `openWorkspace`), so
 * the first forge request is already intercepted.
 *
 * A value may be a fixture or a thunk returning one — use a thunk to
 * change the response between polls (e.g. checks queued → passed).
 */
export type ForgeSubject<T> = T | (() => T);

export type ForgeMocks = {
	// `forge_info` / `forge_compare_branch_url` resolve to null when the
	// project's remote isn't a recognised forge — allow null so negative
	// cases can be exercised.
	forge_info?: ForgeSubject<ForgeInfo | null>;
	list_reviews?: ForgeSubject<ForgeReview[]>;
	get_review?: ForgeSubject<ForgeReview>;
	get_review_merge_status?: ForgeSubject<ReviewMergeStatus>;
	get_repo_info?: ForgeSubject<RepoInfo>;
	list_ci_checks?: ForgeSubject<CiCheck[]>;
	get_review_base_repo_url?: ForgeSubject<string | null>;
	forge_compare_branch_url?: ForgeSubject<string | null>;
};

export async function mockForge(page: Page, mocks: ForgeMocks): Promise<void> {
	for (const [command, subject] of Object.entries(mocks)) {
		if (subject === undefined) continue;
		await page.route(`**/${command}`, async (route: Route) => {
			const resolved = typeof subject === "function" ? (subject as () => unknown)() : subject;
			await route.fulfill({
				status: 200,
				contentType: "application/json",
				body: JSON.stringify({ type: "success", subject: resolved }),
			});
		});
	}
}

/** A GitHub `ForgeInfo`, with every capability on by default. */
export function githubForgeInfo(overrides: Partial<ForgeInfo> = {}): ForgeInfo {
	return {
		name: "github",
		baseUrl: "https://github.com/acme/widgets",
		commitUrlPath: "/commit/",
		prUrlPath: "/pull/",
		unit: { name: "Pull request", abbr: "PR", symbol: "#" },
		posthogLabel: "PR",
		capabilities: { checks: true, repoInfo: true, prService: true, listService: true },
		...overrides,
	};
}

/** A GitLab `ForgeInfo` (MR labels, no checks backend). */
export function gitlabForgeInfo(overrides: Partial<ForgeInfo> = {}): ForgeInfo {
	return {
		name: "gitlab",
		baseUrl: "https://gitlab.com/acme/widgets",
		commitUrlPath: "/-/commit/",
		prUrlPath: "/-/merge_requests/",
		unit: { name: "Merge request", abbr: "MR", symbol: "!" },
		posthogLabel: "Gitlab MR",
		capabilities: { checks: false, repoInfo: true, prService: true, listService: true },
		...overrides,
	};
}

/** A minimal open `ForgeReview` for `branchName` (PR/MR `number`). */
export function forgeReview(
	number: number,
	branchName: string,
	overrides: Partial<ForgeReview> = {},
): ForgeReview {
	return {
		htmlUrl: `https://github.com/acme/widgets/pull/${number}`,
		number,
		title: `Review for ${branchName}`,
		body: null,
		author: null,
		labels: [],
		draft: false,
		sourceBranch: branchName,
		targetBranch: "master",
		sha: "0".repeat(40),
		createdAt: "2026-06-01T00:00:00Z",
		modifiedAt: "2026-06-01T00:00:00Z",
		mergedAt: null,
		closedAt: null,
		repositorySshUrl: null,
		repositoryHttpsUrl: null,
		repoOwner: null,
		headRepoIsFork: false,
		reviewers: [],
		unitSymbol: "#",
		lastSyncAt: "2026-06-01T00:00:00Z",
		...overrides,
	};
}

/** A single CI check run with a terminal `conclusion`. */
export function ciCheck(
	name: string,
	conclusion: CiConclusion,
	overrides: Partial<CiCheck> = {},
): CiCheck {
	return {
		id: name.length, // any stable number; identity isn't asserted
		name,
		output: { summary: "", text: "", title: "" },
		startedAt: "2026-06-01T00:00:00Z",
		status: { complete: { conclusion, completed_at: "2026-06-01T00:05:00Z" } },
		headSha: "0".repeat(40),
		url: "",
		htmlUrl: "",
		detailsUrl: "",
		pullRequests: [],
		reference: "refs/heads/branch",
		lastSyncAt: "2026-06-01T00:05:00Z",
		...overrides,
	};
}

/** A still-running (non-terminal) CI check run. */
export function ciCheckRunning(name: string, overrides: Partial<CiCheck> = {}): CiCheck {
	return ciCheck(name, "success", { status: "inProgress", ...overrides });
}

export function mergeStatus(overrides: Partial<ReviewMergeStatus> = {}): ReviewMergeStatus {
	return { mergeableState: "clean", commentsCount: 0, isMergeable: true, ...overrides };
}

/**
 * A non-fork repo the caller can push to (so `canMerge` is true). This is
 * the realistic default whenever `capabilities.repoInfo` is on — without
 * it the merge button trips the "requires push permissions" gate.
 */
export function repoInfo(overrides: Partial<RepoInfo> = {}): RepoInfo {
	return {
		permissions: { admin: true, maintain: true, push: true, triage: true, pull: true },
		fork: false,
		deleteBranchOnMerge: false,
		...overrides,
	};
}
