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
import { createApi, fakeBaseQuery } from "@reduxjs/toolkit/query/react";
import type { ForgeReview, TreeChange } from "@gitbutler/but-sdk";

export type QueryTag =
	| "AbsorptionPlan"
	| "BranchDetails"
	| "BranchDiff"
	| "Branches"
	| "ChangesInWorktree"
	| "CIChecks"
	| "CommitDetailsWithLineStats"
	| "DraftPR"
	| "DryRun"
	| "Editors"
	| "ForgeInfo"
	| "GUISettings"
	| "HeadInfo"
	| "Projects"
	| "Review"
	| "ReviewMergeStatus"
	| "Reviews"
	| "TreeChangeDiffs";

export const queryResult = async <T>(request: () => Promise<T>) => {
	try {
		return { data: await request() };
	} catch (error) {
		return { error };
	}
};

const reviewId = ({ projectId, reviewId }: GetReviewParams): string => `${projectId}:${reviewId}`;

export const liteApi = createApi({
	reducerPath: "liteApi",
	baseQuery: fakeBaseQuery<unknown>(),
	tagTypes: [
		"AbsorptionPlan",
		"BranchDetails",
		"BranchDiff",
		"Branches",
		"ChangesInWorktree",
		"CIChecks",
		"CommitDetailsWithLineStats",
		"DraftPR",
		"DryRun",
		"Editors",
		"ForgeInfo",
		"GUISettings",
		"HeadInfo",
		"Projects",
		"Review",
		"ReviewMergeStatus",
		"Reviews",
		"TreeChangeDiffs",
	],
	keepUnusedDataFor: 300,
	endpoints: (builder) => ({
		branchDetails: builder.query<
			Awaited<ReturnType<typeof window.lite.branchDetails>>,
			BranchDetailsParams
		>({
			queryFn: (params) => queryResult(() => window.lite.branchDetails(params)),
			providesTags: (_data, _error, { projectId }) => [{ type: "BranchDetails", id: projectId }],
		}),
		branchDiff: builder.query<Awaited<ReturnType<typeof window.lite.branchDiff>>, BranchDiffParams>(
			{
				queryFn: (params) => queryResult(() => window.lite.branchDiff(params)),
				providesTags: (_data, _error, { projectId }) => [{ type: "BranchDiff", id: projectId }],
			},
		),
		changesInWorktree: builder.query<
			Awaited<ReturnType<typeof window.lite.changesInWorktree>>,
			string
		>({
			queryFn: (projectId) => queryResult(() => window.lite.changesInWorktree(projectId)),
			providesTags: (_data, _error, projectId) => [{ type: "ChangesInWorktree", id: projectId }],
		}),
		commitDetailsWithLineStats: builder.query<
			Awaited<ReturnType<typeof window.lite.commitDetailsWithLineStats>>,
			CommitDetailsWithLineStatsParams
		>({
			queryFn: (params) => queryResult(() => window.lite.commitDetailsWithLineStats(params)),
			providesTags: (_data, _error, { projectId }) => [
				{ type: "CommitDetailsWithLineStats", id: projectId },
			],
		}),
		forgeInfo: builder.query<Awaited<ReturnType<typeof window.lite.forgeInfo>>, string>({
			queryFn: (projectId) => queryResult(() => window.lite.forgeInfo(projectId)),
			providesTags: (_data, _error, projectId) => [{ type: "ForgeInfo", id: projectId }],
		}),
		headInfo: builder.query<Awaited<ReturnType<typeof window.lite.headInfo>>, string>({
			queryFn: (projectId) => queryResult(() => window.lite.headInfo(projectId)),
			providesTags: (_data, _error, projectId) => [{ type: "HeadInfo", id: projectId }],
		}),
		getReview: builder.query<Awaited<ReturnType<typeof window.lite.getReview>>, GetReviewParams>({
			queryFn: (params) => queryResult(() => window.lite.getReview(params)),
			providesTags: (_data, _error, params) => [{ type: "Review", id: reviewId(params) }],
		}),
		getReviewMergeStatus: builder.query<
			Awaited<ReturnType<typeof window.lite.getReviewMergeStatus>>,
			GetReviewParams
		>({
			queryFn: (params) => queryResult(() => window.lite.getReviewMergeStatus(params)),
			providesTags: (_data, _error, params) => [
				{ type: "ReviewMergeStatus", id: reviewId(params) },
			],
		}),
		listReviews: builder.query<
			{
				reviews: Awaited<ReturnType<typeof window.lite.listReviews>>;
				reviewsBySourceBranch: Record<string, ForgeReview>;
			} | null,
			ListReviewsParams
		>({
			queryFn: async (params) => {
				try {
					const reviews = await window.lite.listReviews(params);
					return {
						data: {
							reviews,
							reviewsBySourceBranch: Object.fromEntries(
								reviews.map((review) => [review.sourceBranch, review]),
							),
						},
					};
				} catch (error) {
					// listReviews throws when a forge cannot be determined.
					// oxlint-disable-next-line no-console
					console.warn(error);
					return { data: null };
				}
			},
			providesTags: (_data, _error, { projectId }) => [{ type: "Reviews", id: projectId }],
		}),
		listBranches: builder.query<
			Awaited<ReturnType<typeof window.lite.listBranches>>,
			ListBranchesParams
		>({
			queryFn: ({ projectId, filter }) =>
				queryResult(() => window.lite.listBranches(projectId, filter)),
			providesTags: (_data, _error, { projectId }) => [{ type: "Branches", id: projectId }],
		}),
		listProjects: builder.query<
			Awaited<ReturnType<typeof window.lite.listProjectsStateless>>,
			void
		>({
			queryFn: () => queryResult(() => window.lite.listProjectsStateless()),
			providesTags: ["Projects"],
		}),
		listEditors: builder.query<Awaited<ReturnType<typeof window.lite.listEditors>>, void>({
			queryFn: () => queryResult(() => window.lite.listEditors()),
			providesTags: ["Editors"],
		}),
		listCIChecks: builder.query<
			{
				data: Awaited<ReturnType<typeof window.lite.listCiChecks>>;
				aggregate: ReturnType<typeof aggregateCIChecks>;
			},
			Omit<ListCiChecksParams, "cacheConfig">
		>({
			queryFn: async ({ projectId, reference }) => {
				try {
					const data = await window.lite.listCiChecks({
						projectId,
						reference,
						cacheConfig: "noCache",
					});
					return { data: { data, aggregate: aggregateCIChecks(data) } };
				} catch {
					return { data: { data: [], aggregate: null } };
				}
			},
			providesTags: (_data, _error, { projectId }) => [{ type: "CIChecks", id: projectId }],
		}),
		treeChangeDiffs: builder.query<
			Awaited<ReturnType<typeof window.lite.treeChangeDiffs>>,
			TreeChangeDiffParams
		>({
			queryFn: (params) => queryResult(() => window.lite.treeChangeDiffs(params)),
			providesTags: (_data, _error, { projectId }) => [{ type: "TreeChangeDiffs", id: projectId }],
		}),
		treeChangeDiffsBatch: builder.query<
			Array<Awaited<ReturnType<typeof window.lite.treeChangeDiffs>>>,
			{ projectId: string; changes: Array<TreeChange> }
		>({
			queryFn: ({ projectId, changes }) =>
				queryResult(() =>
					Promise.all(changes.map((change) => window.lite.treeChangeDiffs({ projectId, change }))),
				),
			providesTags: (_data, _error, { projectId }) => [{ type: "TreeChangeDiffs", id: projectId }],
		}),
		absorptionPlan: builder.query<
			Awaited<ReturnType<typeof window.lite.absorptionPlan>>,
			AbsorptionPlanParams
		>({
			queryFn: (params) => queryResult(() => window.lite.absorptionPlan(params)),
			providesTags: (_data, _error, { projectId }) => [{ type: "AbsorptionPlan", id: projectId }],
		}),
		getGUISettings: builder.query<Awaited<ReturnType<typeof window.lite.readGUISettings>>, void>({
			queryFn: () => queryResult(() => window.lite.readGUISettings()),
			providesTags: ["GUISettings"],
		}),
	}),
});

export const {
	useAbsorptionPlanQuery,
	useBranchDetailsQuery,
	useBranchDiffQuery,
	useChangesInWorktreeQuery,
	useCommitDetailsWithLineStatsQuery,
	useForgeInfoQuery,
	useGetGUISettingsQuery,
	useGetReviewMergeStatusQuery,
	useHeadInfoQuery,
	useListBranchesQuery,
	useListEditorsQuery,
	useListProjectsQuery,
	useListReviewsQuery,
	useTreeChangeDiffsBatchQuery,
} = liteApi;

const checksPollingInterval = (
	polling: "passive" | "priority",
	status: NonNullable<ReturnType<typeof aggregateCIChecks>>["status"] | undefined,
): number => {
	const priority = polling === "priority";
	switch (status) {
		case "in_progress":
			return priority ? 5_000 : 15_000;
		case "action_required":
			return priority ? 10_000 : 45_000;
		default:
			return priority ? 20_000 : 120_000;
	}
};

export const useListCIChecksQuery = ({
	polling,
	skip = false,
	...params
}: Omit<ListCiChecksParams, "cacheConfig"> & {
	polling: "passive" | "priority";
	skip?: boolean;
}) => {
	const state = liteApi.endpoints.listCIChecks.useQueryState(params, { skip });
	const subscription = liteApi.endpoints.listCIChecks.useQuerySubscription(params, {
		pollingInterval: checksPollingInterval(polling, state.data?.aggregate?.status),
		refetchOnFocus: polling === "priority",
		skip,
	});

	return { ...state, ...subscription };
};
