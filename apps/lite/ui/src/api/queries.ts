import type {
	BranchDetailsParams,
	BranchDiffParams,
	CommitDetailsWithLineStatsParams,
	TreeChangeDiffParams,
} from "#electron/ipc.ts";
import { queryOptions } from "@tanstack/react-query";

export enum QueryKey {
	BranchDetails = "branchDetails",
	BranchDiff = "branchDiff",
	ChangesInWorktree = "changesInWorktree",
	CommitDetailsWithLineStats = "commitDetailsWithLineStats",
	HeadInfo = "headInfo",
	Branches = "branches",
	Projects = "projects",
	TreeChangeDiffs = "treeChangeDiffs",
}

export const branchDetailsQueryOptions = (params: BranchDetailsParams) =>
	queryOptions({
		queryKey: [QueryKey.BranchDetails, params],
		queryFn: () => window.lite.branchDetails(params),
	});

export const branchDiffQueryOptions = (params: BranchDiffParams) =>
	queryOptions({
		queryKey: [QueryKey.BranchDiff, params],
		queryFn: () => window.lite.branchDiff(params),
	});

export const changesInWorktreeQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [QueryKey.ChangesInWorktree, projectId],
		queryFn: () => window.lite.changesInWorktree(projectId),
	});

export const commitDetailsWithLineStatsQueryOptions = (params: CommitDetailsWithLineStatsParams) =>
	queryOptions({
		queryKey: [QueryKey.CommitDetailsWithLineStats, params],
		queryFn: () => window.lite.commitDetailsWithLineStats(params),
	});

export const headInfoQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [QueryKey.HeadInfo, projectId],
		queryFn: () => window.lite.headInfo(projectId),
	});

export const listBranchesQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [QueryKey.Branches, projectId],
		queryFn: () => window.lite.listBranches(projectId, null),
	});

export const listProjectsQueryOptions = () =>
	queryOptions({
		queryKey: [QueryKey.Projects],
		queryFn: () => window.lite.listProjects(),
	});

export const treeChangeDiffsQueryOptions = (params: TreeChangeDiffParams) => {
	const { projectId, change } = params;
	return queryOptions({
		queryKey: [QueryKey.TreeChangeDiffs, projectId, change],
		queryFn: () => window.lite.treeChangeDiffs({ projectId, change }),
	});
};
