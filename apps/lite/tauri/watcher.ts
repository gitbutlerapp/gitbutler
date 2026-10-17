import type { WatcherPayload } from "@gitbutler/but-sdk";
import type { WatcherSubscribeParams, WatcherSubscribeResult } from "#electron/ipc.ts";
import { backend, listenNow, type Subscribe } from "./backend.ts";

// Tauri emits the Svelte app's raw payloads; Lite reads the SDK's tagged ones.
// No changed paths are sent, which Lite treats as "unknown, refresh broadly".
const watcherPayloadByEvent: Record<string, (payload: never) => WatcherPayload> = {
	"git/fetch": () => ({ type: "gitFetch", subject: null }),
	"git/head": (subject) => ({ type: "gitHead", subject }),
	"git/activity": (subject) => ({ type: "gitActivity", subject }),
	"workspace-activity": () => ({ type: "workspaceActivity", subject: null }),
	"external-invalidation": (subject) => ({ type: "externalInvalidation", subject }),
	worktree_changes: (changes) => ({
		type: "worktreeChanges",
		subject: { changes, changedPaths: [] },
	}),
	linked_worktree_changes: (subject: { worktree: string; changes: never }) => ({
		type: "linkedWorktreeChanges",
		subject: { ...subject, changedPaths: [] },
	}),
};

const channelPrefix = "watcher:";

export const watcherChannel = (channel: string): Subscribe | undefined => {
	if (!channel.startsWith(channelPrefix)) return undefined;
	const projectId = channel.slice(channelPrefix.length);
	return (listener) => {
		const unsubscribers = Object.entries(watcherPayloadByEvent).map(([suffix, toPayload]) => {
			const name = `project://${projectId}/${suffix}`;
			return listenNow(name, (payload) => listener({ name, payload: toPayload(payload as never) }));
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
