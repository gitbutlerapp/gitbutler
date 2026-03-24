import { changesInWorktreeQueryOptions } from "#ui/api/queries.ts";
import { WatcherEvent } from "@gitbutler/but-sdk";
import { QueryClient } from "@tanstack/react-query";

export function handleWatcher(event: WatcherEvent, projectId: string, client: QueryClient): void {
	switch (event.payload.type) {
		case "gitFetch":
		case "gitHead":
		case "gitActivity":
			break;
		case "worktreeChanges":
			const workspaceChanges = event.payload.subject.changes;
			client.setQueryData(
				changesInWorktreeQueryOptions(projectId).queryKey,
				() => workspaceChanges,
			);
			break;
	}
}
