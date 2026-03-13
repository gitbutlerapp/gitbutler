import type {
	BranchDetailsParams,
	BranchDiffParams,
	CommitDetailsWithLineStatsParams,
	TreeChangeDiffParams,
} from "#electron/ipc";
import { WatcherEvent } from "@gitbutler/but-sdk";
import { QueryClient, queryOptions, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

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

export function useWatcher(projectId: string) {
	const client = useQueryClient();
	useEffect(() => {
		const subscriptionPromise = window.lite.watcherSubscribe(projectId, (event) =>
			handleWatcher(event, projectId, client),
		);

		void subscriptionPromise.catch((error) => {
			// oxlint-disable-next-line no-console
			console.warn("Failed to subscribe to watcher", error);
		});

		return () => {
			void subscriptionPromise
				.then((subscriptionId) => window.lite.watcherUnsubscribe(subscriptionId))
				.catch((error) => {
					// oxlint-disable-next-line no-console
					console.warn("Failed to unsubscribe from watcher", error);
				});
		};
	}, [client, projectId]);
}

function handleWatcher(event: WatcherEvent, projectId: string, client: QueryClient): boolean {
	switch (event.payload.type) {
		case "gitFetch":
		case "gitHead":
		case "gitActivity":
			return false;
		case "worktreeChanges":
			const opts = changesInWorktreeQueryOptions(projectId);
			const workspaceChanges = event.payload.subject.changes;
			client.setQueryData(opts.queryKey, () => workspaceChanges);
			return true;
	}
}
