import type {
	AbsorptionPlanParams,
	BranchDetailsParams,
	BranchDiffParams,
	CommitDetailsWithLineStatsParams,
	GetReviewParams,
	ListBranchesParams,
	TreeChangeDiffParams,
} from "#electron/ipc.ts";
import { queryOptions } from "@tanstack/react-query";

export enum QueryKey {
	BranchDetails = "branchDetails",
	BranchDiff = "branchDiff",
	ChangesInWorktree = "changesInWorktree",
	CommitDetailsWithLineStats = "commitDetailsWithLineStats",
	HeadInfo = "headInfo",
	Review = "review",
	Branches = "branches",
	Editors = "editors",
	Projects = "projects",
	TreeChangeDiffs = "treeChangeDiffs",
	AbsorptionPlan = "absorptionPlan",
	DryRun = "dryRun",
}

export const branchDetailsQueryOptions = ({ projectId, ...params }: BranchDetailsParams) =>
	queryOptions({
		queryKey: [QueryKey.BranchDetails, projectId, params],
		queryFn: () => window.lite.branchDetails({ projectId, ...params }),
	});

export const branchDiffQueryOptions = ({ projectId, ...params }: BranchDiffParams) =>
	queryOptions({
		queryKey: [QueryKey.BranchDiff, projectId, params],
		queryFn: () => window.lite.branchDiff({ projectId, ...params }),
	});

export const changesInWorktreeQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [QueryKey.ChangesInWorktree, projectId],
		queryFn: () => window.lite.changesInWorktree(projectId),
	});

export const commitDetailsWithLineStatsQueryOptions = ({
	projectId,
	...params
}: CommitDetailsWithLineStatsParams) =>
	queryOptions({
		queryKey: [QueryKey.CommitDetailsWithLineStats, projectId, params],
		queryFn: () => window.lite.commitDetailsWithLineStats({ projectId, ...params }),
	});

export const headInfoQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [QueryKey.HeadInfo, projectId],
		queryFn: () => window.lite.headInfo(projectId),
	});

export const getReviewQueryOptions = ({ projectId, reviewId }: GetReviewParams) =>
	queryOptions({
		queryKey: [QueryKey.Review, projectId, reviewId],
		queryFn: () => window.lite.getReview({ projectId, reviewId }),
	});

/** @public */
export const listBranchesQueryOptions = ({ projectId, ...params }: ListBranchesParams) =>
	queryOptions({
		queryKey: [QueryKey.Branches, projectId, params],
		queryFn: () => window.lite.listBranches(projectId, params.filter),
	});

export const listProjectsQueryOptions = queryOptions({
	queryKey: [QueryKey.Projects],
	queryFn: () => window.lite.listProjectsStateless(),
});

export const listEditorsQueryOptions = queryOptions({
	queryKey: [QueryKey.Editors],
	queryFn: () => window.lite.listEditors(),
});

export const treeChangeDiffsQueryOptions = ({ projectId, change }: TreeChangeDiffParams) =>
	queryOptions({
		queryKey: [QueryKey.TreeChangeDiffs, projectId, change],
		queryFn: () => window.lite.treeChangeDiffs({ projectId, change }),
	});

export const absorptionPlanQueryOptions = ({ projectId, target }: AbsorptionPlanParams) =>
	queryOptions({
		queryKey: [QueryKey.AbsorptionPlan, projectId, target],
		queryFn: () => window.lite.absorptionPlan({ projectId, target }),
	});
