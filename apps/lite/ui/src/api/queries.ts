import type {
	AbsorptionPlanParams,
	BranchDetailsParams,
	BranchDiffParams,
	CommitDetailsWithLineStatsParams,
	GetReviewParams,
	ListCiChecksParams,
	ListReviewsParams,
	TreeChangeDiffParams,
} from "#electron/ipc.ts";
import { aggregateCIChecks } from "#ui/ci.ts";
import { clampAutoFetch, defaultSettings } from "#ui/settings.ts";
import type { ForgeReview } from "@gitbutler/but-sdk";
import {
	infiniteQueryOptions,
	queryOptions,
	skipToken,
	type QueryClient,
} from "@tanstack/react-query";
import * as ms from "ms";

export type QueryKey =
	| "branchDetails"
	| "branchDiff"
	| "branchList"
	| "changesInWorktree"
	| "ciChecks"
	| "comments"
	| "commitDetailsWithLineStats"
	| "forgeInfo"
	| "headInfo"
	| "currentForgeLogin"
	| "repoLabels"
	| "review"
	| "reviewComments"
	| "reviewSubmissions"
	| "reviewTimelineEvents"
	| "reviewReactions"
	| "commentReactions"
	| "reviewMergeStatus"
	| "reviewerCandidates"
	| "reviews"
	| "editors"
	| "projects"
	| "treeChangeDiffs"
	| "absorptionPlan"
	| "dryRun"
	| "guiSettings"
	| "workspaceFetch"
	| "workspaceFetchStatus"
	| "workspaceTargetCommits"
	| "workspaceTargetCommitsOlder";

export const branchDetailsQueryOptions = ({ projectId, ...params }: BranchDetailsParams) =>
	queryOptions({
		queryKey: ["branchDetails" satisfies QueryKey, projectId, params],
		queryFn: () => window.lite.branchDetails({ projectId, ...params }),
	});

export const branchDiffQueryOptions = ({ projectId, ...params }: BranchDiffParams) =>
	queryOptions({
		queryKey: ["branchDiff" satisfies QueryKey, projectId, params],
		queryFn: () => window.lite.branchDiff({ projectId, ...params }),
	});

export const branchListQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["branchList" satisfies QueryKey, projectId],
		queryFn: () => window.lite.branchList(projectId),
	});

export const changesInWorktreeQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["changesInWorktree" satisfies QueryKey, projectId],
		queryFn: () => window.lite.changesInWorktree(projectId),
	});

export const commentsQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["comments" satisfies QueryKey, projectId],
		queryFn: () => window.lite.commentsList(projectId),
	});

export const commitDetailsWithLineStatsQueryOptions = ({
	projectId,
	...params
}: CommitDetailsWithLineStatsParams) =>
	queryOptions({
		queryKey: ["commitDetailsWithLineStats" satisfies QueryKey, projectId, params],
		queryFn: () => window.lite.commitDetailsWithLineStats({ projectId, ...params }),
	});

export const forgeInfoOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["forgeInfo" satisfies QueryKey, projectId],
		queryFn: () => window.lite.forgeInfo(projectId),
	});

export const headInfoQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["headInfo" satisfies QueryKey, projectId],
		queryFn: () => window.lite.headInfo(projectId),
	});

export const getReviewQueryOptions = ({ projectId, reviewId }: GetReviewParams) =>
	queryOptions({
		queryKey: ["review" satisfies QueryKey, projectId, reviewId],
		queryFn: () => window.lite.getReview({ projectId, reviewId }),
	});

export const workspaceTargetCommitsQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["workspaceTargetCommits" satisfies QueryKey, projectId],
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

const olderTargetCommitsPageSize = 25;

/**
 * Pages of target history older than the workspace's fork point, continued
 * below `from` with a commit-id cursor. A `null` cursor means the base
 * listing has not arrived yet, so there is nothing to continue from.
 */
export const olderTargetCommitsInfiniteQueryOptions = (projectId: string, from: string | null) =>
	infiniteQueryOptions({
		queryKey: ["workspaceTargetCommitsOlder" satisfies QueryKey, projectId, from],
		queryFn:
			from === null
				? skipToken
				: ({ pageParam }) =>
						window.lite.workspaceTargetCommits({
							projectId,
							from: pageParam,
							limit: olderTargetCommitsPageSize,
						}),
		initialPageParam: from ?? "",
		getNextPageParam: (lastPage) =>
			lastPage.hasMore ? (lastPage.commits.at(-1)?.commit.id ?? undefined) : undefined,
	});

export const workspaceFetchStatusQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["workspaceFetchStatus" satisfies QueryKey, projectId],
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
		queryKey: ["workspaceFetch" satisfies QueryKey, projectId],
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

/** This query should be gated by PR capability lest it fail. */
export const listReviewCommentsQueryOptions = ({ projectId, reviewId }: GetReviewParams) =>
	queryOptions({
		queryKey: ["reviewComments" satisfies QueryKey, projectId, reviewId],
		queryFn: () => window.lite.listReviewComments({ projectId, reviewId }),
		// Fresh forge fetch each time; keep a gentle poll while the tab is open
		// so replies from others appear without a manual refresh.
		staleTime: 60_000,
		refetchInterval: 60_000,
	});

export const currentForgeLoginQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["currentForgeLogin" satisfies QueryKey, projectId],
		queryFn: () => window.lite.currentForgeLogin(projectId),
		// Resolved from local account storage; changes only on re-auth.
		staleTime: Number.POSITIVE_INFINITY,
	});

/** Gate on the forge being GitHub; other forges reject this call. */
export const repoLabelsQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["repoLabels" satisfies QueryKey, projectId],
		queryFn: () => window.lite.listRepoLabels(projectId),
		// Label definitions rarely change.
		staleTime: 5 * 60_000,
	});

/** Gate on the forge being GitHub; other forges reject this call. */
export const reviewerCandidatesQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["reviewerCandidates" satisfies QueryKey, projectId],
		queryFn: () => window.lite.listReviewerCandidates(projectId),
		// Collaborator lists rarely change.
		staleTime: 5 * 60_000,
	});

/** This query should be gated by PR capability lest it fail. */
export const listReviewSubmissionsQueryOptions = ({ projectId, reviewId }: GetReviewParams) =>
	queryOptions({
		queryKey: ["reviewSubmissions" satisfies QueryKey, projectId, reviewId],
		queryFn: () => window.lite.listReviewSubmissions({ projectId, reviewId }),
		// Same freshness posture as the comments: fresh fetch, gentle poll.
		staleTime: 60_000,
		refetchInterval: 60_000,
	});

/** This query should be gated by PR capability lest it fail. */
export const listReviewTimelineEventsQueryOptions = ({ projectId, reviewId }: GetReviewParams) =>
	queryOptions({
		queryKey: ["reviewTimelineEvents" satisfies QueryKey, projectId, reviewId],
		queryFn: () => window.lite.listReviewTimelineEvents({ projectId, reviewId }),
		// Same freshness posture as the comments: fresh fetch, gentle poll.
		staleTime: 60_000,
		refetchInterval: 60_000,
	});

/** This query should be gated by PR capability lest it fail. */
export const listReviewReactionsQueryOptions = ({ projectId, reviewId }: GetReviewParams) =>
	queryOptions({
		queryKey: ["reviewReactions" satisfies QueryKey, projectId, reviewId],
		queryFn: () => window.lite.listReviewReactions({ projectId, reviewId }),
		// Same freshness posture as the comments: fresh fetch, gentle poll.
		staleTime: 60_000,
		refetchInterval: 60_000,
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
		queryKey: ["commentReactions" satisfies QueryKey, projectId, commentId],
		queryFn: () => window.lite.listCommentReactions({ projectId, commentId }),
		staleTime: 60_000,
	});

export const getReviewMergeStatusQueryOptions = ({ projectId, reviewId }: GetReviewParams) =>
	queryOptions({
		queryKey: ["reviewMergeStatus" satisfies QueryKey, projectId, reviewId],
		queryFn: () => window.lite.getReviewMergeStatus({ projectId, reviewId }),
		staleTime: ({ state: { data } }) => (data?.isMergeable ? 30_000 : 10_000),
		// Mergeability flips from the forge side (checks finish, approvals
		// land); poll while the tab is open. Pauses when the app is unfocused
		// (refetchIntervalInBackground defaults off), and the focusManager
		// wiring in main.tsx catches up on refocus.
		refetchInterval: 60_000,
	});

/** This query should be gated by PR capability lest it fail. */
export const listReviewsQueryOptions = ({ projectId, ...params }: ListReviewsParams) =>
	queryOptions({
		queryKey: ["reviews" satisfies QueryKey, projectId, params],
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

export const listProjectsQueryOptions = queryOptions({
	queryKey: ["projects" satisfies QueryKey],
	queryFn: () => window.lite.listProjectsStateless(),
});

export const listEditorsQueryOptions = queryOptions({
	queryKey: ["editors" satisfies QueryKey],
	queryFn: () => window.lite.listEditors(),
});

/** This query should be gated by checks capability. */
// There is no watcher event that could invalidate this query.
export const listCIChecksQueryOptions = ({
	projectId,
	reference,
	polling,
}: Omit<ListCiChecksParams, "cacheConfig"> & {
	polling: "passive" | "priority";
}) =>
	queryOptions({
		queryKey: ["ciChecks" satisfies QueryKey, projectId, reference],
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

export const treeChangeDiffsQueryOptions = ({ projectId, change }: TreeChangeDiffParams) =>
	queryOptions({
		queryKey: ["treeChangeDiffs" satisfies QueryKey, projectId, change],
		queryFn: () => window.lite.treeChangeDiffs({ projectId, change }),
	});

export const absorptionPlanQueryOptions = ({ projectId, target }: AbsorptionPlanParams) =>
	queryOptions({
		queryKey: ["absorptionPlan" satisfies QueryKey, projectId, target],
		queryFn: () => window.lite.absorptionPlan({ projectId, target }),
	});

export const guiSettingsQueryOptions = queryOptions({
	queryKey: ["guiSettings" satisfies QueryKey],
	queryFn: () => window.lite.readGUISettings(),
});
