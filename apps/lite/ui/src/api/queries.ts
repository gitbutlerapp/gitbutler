import type {
	AbsorptionPlanParams,
	BranchDetailsParams,
	BranchDiffParams,
	CommitDetailsWithLineStatsParams,
	GetReviewParams,
	ListBranchesParams,
	ListCiChecksParams,
	ListReviewsParams,
	TreeChangeDiffParams,
} from "#electron/ipc.ts";
import { aggregateCIChecks } from "#ui/ci.ts";
import type { ForgeReview } from "@gitbutler/but-sdk";
import { queryOptions } from "@tanstack/react-query";

export type QueryKey =
	| "branchDetails"
	| "branchDiff"
	| "changesInWorktree"
	| "ciChecks"
	| "commitDetailsWithLineStats"
	| "forgeInfo"
	| "headInfo"
	| "review"
	| "reviewMergeStatus"
	| "reviews"
	| "branches"
	| "editors"
	| "projects"
	| "treeChangeDiffs"
	| "absorptionPlan"
	| "dryRun"
	| "guiSettings";

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

export const changesInWorktreeQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["changesInWorktree" satisfies QueryKey, projectId],
		queryFn: () => window.lite.changesInWorktree(projectId),
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

export const getReviewMergeStatusQueryOptions = ({ projectId, reviewId }: GetReviewParams) =>
	queryOptions({
		queryKey: ["reviewMergeStatus" satisfies QueryKey, projectId, reviewId],
		queryFn: () => window.lite.getReviewMergeStatus({ projectId, reviewId }),
		staleTime: ({ state: { data } }) => (data?.isMergeable ? 30_000 : 10_000),
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
	});

export const listBranchesQueryOptions = ({ projectId, ...params }: ListBranchesParams) =>
	queryOptions({
		queryKey: ["branches" satisfies QueryKey, projectId, params],
		queryFn: () => window.lite.listBranches(projectId, params.filter),
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
