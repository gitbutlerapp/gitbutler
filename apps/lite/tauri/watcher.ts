import type { WatcherSubscribeParams, WatcherSubscribeResult } from "#electron/ipc.ts";
import { backend, listenNow, type Subscribe } from "./backend.ts";

const topics = [
	"git/fetch",
	"git/head",
	"git/activity",
	"workspace-activity",
	"external-invalidation",
	"worktree_changes",
	"linked_worktree_changes",
];

const channelPrefix = "watcher:";

export const watcherChannel = (channel: string): Subscribe | undefined => {
	if (!channel.startsWith(channelPrefix)) return undefined;
	const projectId = channel.slice(channelPrefix.length);
	return (listener) => {
		const unsubscribers = topics.map((topic) => {
			const name = `project://${projectId}/${topic}`;
			return listenNow(name, (payload) => listener({ name, payload }));
		});
		return () => {
			for (const unsubscribe of unsubscribers) unsubscribe();
		};
	};
};

export const handlers = {
	watcherSubscribe: async ({
		projectId,
	}: WatcherSubscribeParams): Promise<WatcherSubscribeResult> => {
		await backend("set_project_active", { id: projectId });
		return { subscriptionId: crypto.randomUUID(), eventChannel: `${channelPrefix}${projectId}` };
	},
	// The window's watcher follows its active project, not its subscriptions.
	watcherUnsubscribe: () => true,
	watcherStopAll: () => 0,
};
