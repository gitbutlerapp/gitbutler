import type { PayloadFor } from "#electron/ipc.ts";
import { aggregateCIChecks } from "#ui/ci.ts";
import { clampAutoFetch, defaultSettings } from "#ui/settings.ts";
import type { ForgeReview } from "@gitbutler/but-sdk";
import { apiProvides } from "@gitbutler/but-sdk/cache-tags";
import { queryOptions, type QueryClient } from "@tanstack/react-query";
import * as ms from "ms";

/**
 * The project queries are the endpoints declaring `provides` in Rust — using a
 * name the backend doesn't declare is a type error. Keyed `[key, projectId,
 * ...]`; the fixed position is what lets an invalidation reach a whole query
 * root holding nothing but a project id.
 */
export type ProjectQueryKey = keyof typeof apiProvides;

// `Object.keys` erases key types; the record's keys are exactly these.
export const projectQueryKeys = Object.keys(apiProvides) as ReadonlyArray<ProjectQueryKey>;

/** Keyed without a project id, so no project event can invalidate them. */
type GlobalQueryKey =
	| "aiConfiguration"
	| "editors"
	| "terminals"
	| "forgeAccounts"
	| "userProfile"
	| "projects"
	| "guiSettings";

/**
 * Client state kept in the query cache, so nothing declares for them. `dryRun`
 * memoizes an imperative preview: its key carries the operation and changes it
 * was measured against, and nothing refreshes it in place.
 */
type LocalQueryKey =
	| "commitMessageDraft"
	| "dryRun"
	| "prMergeMethod"
	| "prDraft"
	| "projectAiSettings"
	| "reviewedFiles";

export type QueryKey = ProjectQueryKey | GlobalQueryKey | LocalQueryKey;

declare module "@tanstack/react-query" {
	interface Register {
		/**
		 * Every query key in the app starts with one of ours, so a typo is a type
		 * error wherever a key is written — building one, invalidating it, or
		 * reading it back — without each site having to say so.
		 */
		queryKey: readonly [QueryKey, ...ReadonlyArray<unknown>];
	}
}

/**
 * The name the backend would generate for a branch created right now. Used to
 * name the branch a commit is about to create before it exists. Derived from
 * the branch namespace, so the endpoint provides `Branches` and any ref
 * movement refreshes it.
 */
export const branchCannedNameQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["branchCannedName", projectId],
		queryFn: () => window.lite.branchCannedName(projectId),
	});

export const branchDetailsQueryOptions = ({ projectId, ...params }: PayloadFor<"branchDetails">) =>
	queryOptions({
		queryKey: ["branchDetails", projectId, params],
		queryFn: () => window.lite.branchDetails({ projectId, ...params }),
	});

export const branchDiffQueryOptions = ({ projectId, ...params }: PayloadFor<"branchDiff">) =>
	queryOptions({
		queryKey: ["branchDiff", projectId, params],
		queryFn: () => window.lite.branchDiff({ projectId, ...params }),
	});

export const branchListQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["branchList", projectId],
		queryFn: () => window.lite.branchList(projectId),
	});

export const changesInWorktreeQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["changesInWorktree", projectId],
		queryFn: () =>
			window.lite.changesInWorktree({
				projectId,
				changesSource: { type: "head" },
				computeDepsAndAssignments: true,
			}),
	});

export const commentsQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["commentsList", projectId],
		queryFn: () => window.lite.commentsList(projectId),
	});

export const commitDetailsWithLineStatsQueryOptions = ({
	projectId,
	...params
}: PayloadFor<"commitDetailsWithLineStats">) =>
	queryOptions({
		queryKey: ["commitDetailsWithLineStats", projectId, params],
		queryFn: () => window.lite.commitDetailsWithLineStats({ projectId, ...params }),
	});

/**
 * A conflicted commit's conflicts, derived from the trees the commit itself
 * carries — so the answer is immutable per commit id, and an apply that
 * rewrites the commit lands on a different key rather than invalidating this
 * one. Enable it only for commits already known to conflict: the backend
 * answers for any commit, but the round-trip is pure cost otherwise.
 */
export const commitConflictsQueryOptions = ({
	projectId,
	enabled,
	...params
}: PayloadFor<"commitConflicts"> & { enabled: boolean }) =>
	queryOptions({
		queryKey: ["commitConflicts", projectId, params],
		queryFn: () => window.lite.commitConflicts({ projectId, ...params }),
		enabled,
		staleTime: Infinity,
		// A commit whose conflicts have no hunk representation — a binary, a
		// deletion, an oversized file — makes the backend reject the whole
		// commit. That is a property of the commit, so retrying cannot help.
		retry: false,
	});

export const forgeInfoOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["forgeInfo", projectId],
		queryFn: () => window.lite.forgeInfo(projectId),
	});

export const headInfoQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["headInfo", projectId],
		queryFn: () => window.lite.headInfo(projectId),
	});

export const getReviewQueryOptions = ({ projectId, reviewId }: PayloadFor<"getReview">) =>
	queryOptions({
		queryKey: ["getReview", projectId, reviewId],
		queryFn: () => window.lite.getReview({ projectId, reviewId }),
	});

export const workspaceTargetCommitsQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["workspaceTargetCommits", projectId],
		queryFn: () => window.lite.workspaceTargetCommits({ projectId, from: null, limit: null }),
	});

/**
 * A fetch can turn a reviewed branch into an integrated one while the backend
 * forge cache still holds the pre-merge review, leaving the branch unmatched
 * to the commit that landed it in the Upstream tab. Refresh those reviews
 * (repopulating the backend cache) so the target-commit listing can be
 * re-read afterwards. Runs from the fetch watcher, so a review that never
 * resolves is retried at most once per fetch, and the listing itself stays a
 * purely local call. Failures degrade to unannotated commits.
 */
export const refreshIntegratedReviews = async (
	client: QueryClient,
	projectId: string,
): Promise<void> => {
	const headInfo = await client.fetchQuery({ ...headInfoQueryOptions(projectId), staleTime: 0 });
	const reviewIds = new Set(
		headInfo.stacks.flatMap((stack) =>
			stack.segments.flatMap((segment) => {
				const reviewId = segment.metadata?.review.pullRequest;
				return segment.pushStatus === "integrated" && reviewId != null ? [reviewId] : [];
			}),
		),
	);
	await Promise.allSettled(
		[...reviewIds].flatMap((reviewId) => {
			const options = getReviewQueryOptions({ projectId, reviewId });
			return client.getQueryData<ForgeReview>(options.queryKey)?.mergedAt != null
				? []
				: [client.fetchQuery({ ...options, staleTime: Number.POSITIVE_INFINITY })];
		}),
	);
};

export const workspaceFetchStatusQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["workspaceFetchStatus", projectId],
		queryFn: () => window.lite.workspaceFetchStatus(projectId),
	});

export const workspaceFetchQueryOptions = (
	projectId: string,
	autoFetchFrequency = defaultSettings.autoFetchFrequency,
) => {
	// Throws on empty and large strings.
	let autoFetchFrequencyMs: number;
	try {
		autoFetchFrequencyMs = ms.parse(autoFetchFrequency);
	} catch {
		autoFetchFrequencyMs = Number.NaN;
	}

	return queryOptions({
		queryKey: ["workspaceFetchFromRemotes", projectId],
		queryFn: () =>
			window.lite.workspaceFetchFromRemotes({ projectId, action: null }).then(
				// RQ treats undefined results in queries as errors.
				() => null,
			),
		refetchInterval: Number.isNaN(autoFetchFrequencyMs)
			? false
			: clampAutoFetch(autoFetchFrequencyMs),
		refetchIntervalInBackground: true,
		retry: false,
		// Don't fetch on first mount, simplifying the no polling scenario.
		initialData: null,
	});
};

/**
 * Fresh forge fetch each time; keep a gentle poll while the tab is open so
 * changes from others appear without a manual refresh.
 */
const forgePoll = { staleTime: 60_000, refetchInterval: 60_000 };

/** This query should be gated by PR capability lest it fail. */
export const listReviewCommentsQueryOptions = ({
	projectId,
	reviewId,
}: PayloadFor<"listReviewComments">) =>
	queryOptions({
		queryKey: ["listReviewComments", projectId, reviewId],
		queryFn: () => window.lite.listReviewComments({ projectId, reviewId }),
		...forgePoll,
	});

export const gbConfigQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["getGbConfig", projectId],
		queryFn: () => window.lite.getGbConfig(projectId),
	});

/**
 * Whether the repository's signing configuration actually produces a signature.
 * Runs git, so it is asked for on demand rather than polled.
 */
export const signingSettingsQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["checkSigningSettings", projectId],
		queryFn: () => window.lite.checkSigningSettings(projectId),
		enabled: false,
		retry: false,
		staleTime: Number.POSITIVE_INFINITY,
	});

export const currentForgeLoginQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["currentForgeLogin", projectId],
		queryFn: () => window.lite.currentForgeLogin(projectId),
		// Resolved from local account storage; changes only on re-auth.
		staleTime: Number.POSITIVE_INFINITY,
	});

/** Gate on the forge being GitHub; other forges reject this call. */
export const repoLabelsQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["listRepoLabels", projectId],
		queryFn: () => window.lite.listRepoLabels(projectId),
		// Label definitions rarely change.
		staleTime: 5 * 60_000,
	});

/** Gate on the forge being GitHub; other forges reject this call. */
export const reviewerCandidatesQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["listReviewerCandidates", projectId],
		queryFn: () => window.lite.listReviewerCandidates(projectId),
		// Collaborator lists rarely change.
		staleTime: 5 * 60_000,
	});

/** This query should be gated by PR capability lest it fail. */
export const listReviewSubmissionsQueryOptions = ({
	projectId,
	reviewId,
}: PayloadFor<"getReview">) =>
	queryOptions({
		queryKey: ["listReviewSubmissions", projectId, reviewId],
		queryFn: () => window.lite.listReviewSubmissions({ projectId, reviewId }),
		...forgePoll,
	});

/** This query should be gated by PR capability lest it fail. */
export const listReviewTimelineEventsQueryOptions = ({
	projectId,
	reviewId,
}: PayloadFor<"getReview">) =>
	queryOptions({
		queryKey: ["listReviewTimelineEvents", projectId, reviewId],
		queryFn: () => window.lite.listReviewTimelineEvents({ projectId, reviewId }),
		...forgePoll,
	});

/** This query should be gated by PR capability lest it fail. */
export const listReviewReactionsQueryOptions = ({ projectId, reviewId }: PayloadFor<"getReview">) =>
	queryOptions({
		queryKey: ["listReviewReactions", projectId, reviewId],
		queryFn: () => window.lite.listReviewReactions({ projectId, reviewId }),
		...forgePoll,
	});

/**
 * Who reacted to one comment; the caller only mounts this for comments that
 * have reactions, so most comments cost no request. No poll — the counts on
 * the comment itself are the freshness signal.
 */
export const listCommentReactionsQueryOptions = ({
	projectId,
	commentId,
}: {
	projectId: string;
	commentId: number;
}) =>
	queryOptions({
		queryKey: ["listCommentReactions", projectId, commentId],
		queryFn: () => window.lite.listCommentReactions({ projectId, commentId }),
		staleTime: 60_000,
	});

export const getReviewMergeStatusQueryOptions = ({
	projectId,
	reviewId,
}: PayloadFor<"getReview">) =>
	queryOptions({
		queryKey: ["getReviewMergeStatus", projectId, reviewId],
		queryFn: () => window.lite.getReviewMergeStatus({ projectId, reviewId }),
		staleTime: ({ state: { data } }) => (data?.isMergeable ? 30_000 : 10_000),
		// Mergeability flips from the forge side (checks finish, approvals
		// land); poll while the tab is open. Pauses when the app is unfocused
		// (refetchIntervalInBackground defaults off), and the focusManager
		// wiring in main.tsx catches up on refocus.
		refetchInterval: 60_000,
	});

/** This query should be gated by PR capability lest it fail. */
export const listReviewsQueryOptions = ({ projectId, ...params }: PayloadFor<"listReviews">) =>
	queryOptions({
		queryKey: ["listReviews", projectId, params],
		queryFn: () => window.lite.listReviews({ projectId, ...params }),
		select: (reviews) => {
			const reviewsBySourceBranch = new Map<string, ForgeReview>();
			for (const review of reviews) reviewsBySourceBranch.set(review.sourceBranch, review);
			return {
				reviews,
				reviewsBySourceBranch,
			};
		},
		staleTime: 60_000,
		// Review state changes on the forge side too (closed/reopened/merged
		// on the website, labels, review requests). Poll while the app is
		// focused; refetchIntervalInBackground defaults off, so an
		// unfocused app goes quiet and the focusManager wiring in main.tsx
		// refetches on return instead.
		refetchInterval: 60_000,
	});

/**
 * The backend names platforms the way Rust does; electron reports node's names, so
 * `darwin` would match nothing and quietly yield an empty list.
 */
const backendPlatform = (platform: string): string =>
	platform === "darwin" ? "macos" : platform === "win32" ? "windows" : platform;

/** Terminals are per-platform, and the platform cannot change while running. */
export const terminalsQueryOptions = queryOptions({
	queryKey: ["terminals"],
	queryFn: () => window.lite.getTerminalOptionsForPlatform(backendPlatform(window.lite.platform)),
	staleTime: Number.POSITIVE_INFINITY,
});

export const userProfileQueryOptions = queryOptions({
	queryKey: ["userProfile"],
	queryFn: () => window.lite.getUserProfileLocal(),
});

export const aiConfigurationQueryOptions = queryOptions({
	queryKey: ["aiConfiguration"],
	queryFn: () => window.lite.getAiConfiguration(),
});

export const githubAccountsQueryOptions = queryOptions({
	queryKey: ["forgeAccounts", "github"],
	queryFn: () => window.lite.listKnownGithubAccounts(),
});

export const gitlabAccountsQueryOptions = queryOptions({
	queryKey: ["forgeAccounts", "gitlab"],
	queryFn: () => window.lite.listKnownGitlabAccounts(),
});

export const bitbucketAccountsQueryOptions = queryOptions({
	queryKey: ["forgeAccounts", "bitbucket"],
	queryFn: () => window.lite.listKnownBitbucketAccounts(),
});

export const listProjectsQueryOptions = queryOptions({
	queryKey: ["projects"],
	queryFn: () => window.lite.listProjectsStateless(),
});

export const listEditorsQueryOptions = queryOptions({
	queryKey: ["editors"],
	queryFn: () => window.lite.listEditors(),
});

/** This query should be gated by checks capability. */
// There is no watcher event that could invalidate this query.
export const listCIChecksQueryOptions = ({
	projectId,
	reference,
	polling,
}: Omit<PayloadFor<"listCiChecks">, "cacheConfig"> & {
	polling: "passive" | "priority";
}) =>
	queryOptions({
		queryKey: ["listCiChecks", projectId, reference],
		queryFn: async () => {
			// Aggregated data is needed in queryFn to adjust refetching behaviour. Aggregating here, for
			// use as mentioned and also at call sites, is more efficient.
			//
			// listCiChecks will reject with a message citing HTTP 422 once the branch is merged.
			try {
				const data = await window.lite.listCiChecks({
					projectId,
					reference,
					cacheConfig: "noCache",
				});
				return { data, aggregate: aggregateCIChecks(data) };
			} catch {
				return { data: [], aggregate: null };
			}
		},
		// Refetch periodically, being mindful of rate limiting. Similarly tweak stale time for
		// prioritised queries so that fresh data is likely fetched when the user would see/expect it
		// e.g. window refocus.
		refetchInterval: ({ state: { data: checks } }): number => {
			const prio = polling === "priority";

			switch (checks?.aggregate?.status) {
				case "in_progress":
					return prio ? 5_000 : 15_000;
				case "action_required":
					return prio ? 10_000 : 45_000;
				case "success":
				case "cancelled":
				case "failure":
				case "unknown":
				case undefined:
					return prio ? 20_000 : 120_000;
			}
		},
		staleTime: ({ state: { data: checks } }): number => {
			// Our global default.
			if (polling === "passive") return Number.POSITIVE_INFINITY;

			switch (checks?.aggregate?.status) {
				case "in_progress":
					return 5_000;
				case "action_required":
					return 10_000;
				case "success":
				case "cancelled":
				case "failure":
				case "unknown":
				case undefined:
					return 30_000;
			}
		},
	});

export const treeChangeDiffsQueryOptions = ({ projectId, change }: PayloadFor<"treeChangeDiffs">) =>
	queryOptions({
		queryKey: ["treeChangeDiffs", projectId, change],
		queryFn: () => window.lite.treeChangeDiffs({ projectId, change }),
	});

export const absorptionPlanQueryOptions = ({ projectId, target }: PayloadFor<"absorptionPlan">) =>
	queryOptions({
		queryKey: ["absorptionPlan", projectId, target],
		queryFn: () => window.lite.absorptionPlan({ projectId, target }),
	});

export const guiSettingsQueryOptions = queryOptions({
	queryKey: ["guiSettings"],
	queryFn: () => window.lite.readGUISettings(),
});
