import { changesInWorktreeQueryOptions } from "#ui/queries.ts";
import { WatcherEvent } from "@gitbutler/but-sdk";
import { QueryClient } from "@tanstack/react-query";

export async function removeWatcherSubscription(subscriptionId: string) {
	await window.lite.watcherUnsubscribe(subscriptionId);
}

export async function subscribeToProject(projectId: string, client: QueryClient): Promise<string> {
	const subscriptionId = await window.lite.watcherSubscribe(projectId, (event) =>
		handleWatcher(event, projectId, client),
	);
	return subscriptionId;
}

function handleWatcher(event: WatcherEvent, projectId: string, client: QueryClient): boolean {
	switch (event.payload.type) {
		case "gitFetch":
		case "gitHead":
		case "gitActivity":
			return false;
		case "worktreeChanges":
			const workspaceChanges = event.payload.subject.changes;
			client.setQueryData(
				changesInWorktreeQueryOptions(projectId).queryKey,
				() => workspaceChanges,
			);
			return true;
	}
}
