import { changesInWorktreeQueryOptions, QueryKey } from "#ui/api/queries.ts";
import { WatcherEvent } from "@gitbutler/but-sdk";
import { QueryClient } from "@tanstack/react-query";

export const handleWatcher = (
	event: WatcherEvent,
	projectId: string,
	client: QueryClient,
): void => {
	switch (event.payload.type) {
		case "gitActivity":
		case "workspaceActivity": {
			void client.invalidateQueries({ queryKey: [QueryKey.AbsorptionPlan, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.Branches, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.BranchDetails, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.BranchDiff, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.ChangesInWorktree, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.CommitDetailsWithLineStats, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.DryRun, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.HeadInfo, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.TreeChangeDiffs, projectId] });
			break;
		}
		case "worktreeChanges":
			const workspaceChanges = event.payload.subject.changes;
			client.setQueryData(
				changesInWorktreeQueryOptions(projectId).queryKey,
				() => workspaceChanges,
			);
			void client.invalidateQueries({ queryKey: [QueryKey.AbsorptionPlan, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.DryRun, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.TreeChangeDiffs, projectId] });
			break;
	}
};
