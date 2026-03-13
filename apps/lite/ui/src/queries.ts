import type {
	BranchDetailsParams,
	BranchDiffParams,
	CommitDetailsWithLineStatsParams,
	TreeChangeDiffParams,
} from "#electron/ipc";
import { queryOptions } from "@tanstack/react-query";

export const branchDetailsQueryOptions = (params: BranchDetailsParams) =>
	queryOptions({
		queryKey: ["branchDetails", params],
		queryFn: () => window.lite.branchDetails(params),
	});

export const branchDiffQueryOptions = (params: BranchDiffParams) =>
	queryOptions({
		queryKey: ["branchDiff", params],
		queryFn: () => window.lite.branchDiff(params),
	});

export const changesInWorktreeQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["changesInWorktree", projectId],
		queryFn: () => window.lite.changesInWorktree(projectId),
	});

export const commitDetailsWithLineStatsQueryOptions = (params: CommitDetailsWithLineStatsParams) =>
	queryOptions({
		queryKey: ["commitDetailsWithLineStats", params],
		queryFn: () => window.lite.commitDetailsWithLineStats(params),
	});

export const headInfoQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["headInfo", projectId],
		queryFn: () => window.lite.headInfo(projectId),
	});

export const listBranchesQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: ["branches", projectId],
		queryFn: () => window.lite.listBranches(projectId, null),
	});

export const listProjectsQueryOptions = () =>
	queryOptions({
		queryKey: ["projects"],
		queryFn: () => window.lite.listProjects(),
	});

export const treeChangeDiffsQueryOptions = (params: TreeChangeDiffParams) =>
	queryOptions({
		queryKey: ["treeChangeDiffs", params],
		queryFn: () => window.lite.treeChangeDiffs(params),
	});
