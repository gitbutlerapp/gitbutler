import { changesInWorktreeQueryOptions, QueryKey } from "#ui/api/queries.ts";
import { WatcherEvent } from "@gitbutler/but-sdk";
import { QueryClient } from "@tanstack/react-query";

export const handleWatcher = (
	event: WatcherEvent,
	projectId: string,
	client: QueryClient,
): void => {
	switch (event.payload.type) {
		case "worktreeChanges":
			const workspaceChanges = event.payload.subject.changes;
			client.setQueryData(
				changesInWorktreeQueryOptions(projectId).queryKey,
				() => workspaceChanges,
			);
			void client.invalidateQueries({ queryKey: [QueryKey.TreeChangeDiffs, projectId] });
			void client.invalidateQueries({ queryKey: [QueryKey.AbsorptionPlan, projectId] });
			break;
	}
};
