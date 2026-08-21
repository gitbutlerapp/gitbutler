import { randomUUID } from "node:crypto";
import { watcherStart, type WatcherEvent, type WatcherHandle } from "@gitbutler/but-sdk";

/**
 * How the host pushes a watcher event to the panel: the binding routes
 * `channel` to whatever the panel's `transport.subscribe` listens on. A
 * harness that cannot push can back `emit` with polling; the renderer
 * contract stays the same.
 */
export type WatcherEmit = (channel: string, payload: WatcherEvent) => void;

type ProjectWatcher = {
	handle: WatcherHandle;
	subscriptionIds: Set<string>;
};

type Subscription = {
	projectId: string;
	eventChannel: string;
};

/**
 * The electron WatcherManager's job without its window bookkeeping: one SDK
 * watcher per project, fanned out to per-subscription event channels; the
 * last unsubscribe stops the watcher. The binding calls `stopAll` when the
 * panel closes, since no window close exists here to trigger cleanup.
 */
export const createHostWatcher = (emit: WatcherEmit) => {
	const projectWatchers = new Map<string, ProjectWatcher>();
	const pendingProjectWatchers = new Map<string, Promise<ProjectWatcher>>();
	const subscriptions = new Map<string, Subscription>();
	// A watcher can still be starting when the panel closes; without this the
	// handle would arrive after `stopAll` and run on unwatched.
	let stopped = false;

	const refuseWhenStopped = (): void => {
		if (stopped) throw new Error("The watcher host is shutting down");
	};

	const forward = (projectId: string, event: WatcherEvent): void => {
		const projectWatcher = projectWatchers.get(projectId);
		if (!projectWatcher) return;

		for (const subscriptionId of projectWatcher.subscriptionIds) {
			const subscription = subscriptions.get(subscriptionId);
			if (subscription) emit(subscription.eventChannel, event);
		}
	};

	const ensureProjectWatcher = (projectId: string): Promise<ProjectWatcher> => {
		const existing = projectWatchers.get(projectId);
		if (existing) return Promise.resolve(existing);

		const pending = pendingProjectWatchers.get(projectId);
		if (pending) return pending;

		const creation = watcherStart(projectId, (err, event) => {
			if (err) {
				// oxlint-disable-next-line no-console
				console.warn("Watcher callback failed", err);
				return;
			}
			forward(projectId, event);
		})
			.then((handle) => {
				const watcher: ProjectWatcher = { handle, subscriptionIds: new Set() };
				if (stopped) {
					handle.stop();
					return watcher;
				}
				projectWatchers.set(projectId, watcher);
				return watcher;
			})
			.finally(() => {
				pendingProjectWatchers.delete(projectId);
			});

		pendingProjectWatchers.set(projectId, creation);
		return creation;
	};

	const stopProjectWatcher = (projectId: string): void => {
		const projectWatcher = projectWatchers.get(projectId);
		if (!projectWatcher) return;

		try {
			projectWatcher.handle.stop();
		} catch (error) {
			// oxlint-disable-next-line no-console
			console.warn(`Failed to stop project watcher for ${projectId}`, error);
		}
		projectWatchers.delete(projectId);
	};

	return {
		subscribe: async (projectId: string) => {
			// Checked twice, and through a function so each call re-reads it:
			// before, so a late caller never starts an SDK watcher at all; after,
			// for the start that was already in flight, whose subscription would
			// otherwise land in a cleared table that nothing drives.
			refuseWhenStopped();
			const projectWatcher = await ensureProjectWatcher(projectId);
			refuseWhenStopped();

			const subscriptionId = randomUUID();
			const eventChannel = `workspace:watcher-event:${randomUUID()}`;
			subscriptions.set(subscriptionId, { projectId, eventChannel });
			projectWatcher.subscriptionIds.add(subscriptionId);

			return { subscriptionId, eventChannel };
		},
		unsubscribe: (subscriptionId: string): boolean => {
			const subscription = subscriptions.get(subscriptionId);
			if (!subscription) return false;

			subscriptions.delete(subscriptionId);
			const projectWatcher = projectWatchers.get(subscription.projectId);
			if (projectWatcher) {
				projectWatcher.subscriptionIds.delete(subscriptionId);
				if (projectWatcher.subscriptionIds.size === 0) stopProjectWatcher(subscription.projectId);
			}

			return true;
		},
		stopAll: (): number => {
			stopped = true;
			const count = subscriptions.size;
			for (const projectId of projectWatchers.keys()) stopProjectWatcher(projectId);
			subscriptions.clear();
			return count;
		},
	};
};
