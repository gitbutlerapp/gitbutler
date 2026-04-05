import { WatcherEvent, WatcherHandle, watcherStart } from "@gitbutler/but-sdk";
import { randomUUID } from "node:crypto";

type ProjectWatcherState = {
	/**
	 * The handle to the actual project watcher.
	 *
	 * As long as this is held in memory, the watcher lives.
	 */
	handle: WatcherHandle;
	/**
	 * A set of different subscriptions to this watcher.
	 *
	 * Each subscription could mean different windows watching the same project.
	 */
	subscriptionIds: Set<string>;
};

type WatcherSubscription = {
	/**
	 * The ID of the project being subscribed to.
	 */
	projectId: string;
	/**
	 * The window web contents information of the subscriber.
	 */
	sender: Electron.WebContents;
	/**
	 * The web contents ID of the subscriber.
	 *
	 * Each window has an unique ID.
	 */
	senderId: number;
	/**
	 * The event channel for this subscription.
	 */
	eventChannel: string;
};

export default class WatcherManager {
	/**
	 * Watchers and subscribers by project.
	 */
	private projectWatchers: Map<string, ProjectWatcherState>;
	/**
	 * Watcher being created and its subscribers by project.
	 */
	private pendingProjectWatchers: Map<string, Promise<ProjectWatcherState>>;
	/**
	 * Subscriptions by subscription ID.
	 */
	private watcherSubscriptions: Map<string, WatcherSubscription>;
	/**
	 * All the subscription a given sender has.
	 *
	 * This is a map of all subscription IDs to web contents (window) id.
	 */
	private senderSubscriptions: Map<number, Set<string>>;

	private static instance: WatcherManager | null = null;

	private constructor() {
		this.projectWatchers = new Map();
		this.pendingProjectWatchers = new Map();
		this.watcherSubscriptions = new Map();
		this.senderSubscriptions = new Map();
	}

	static getInstance() {
		if (!this.instance) this.instance = new WatcherManager();

		return this.instance;
	}

	/**
	 * Start watching a project and subscribe to it.
	 *
	 * 1. Ensure that there's already a live watcher in the state for the given project ID. Otherwise, create it.
	 * 2. Setup a destruction listener in order to ensure that all subscriptions are closed if the owner window is closed.
	 * 3. Store the subscription information in the state.
	 *
	 * @returns The ID of the subscription and the event channel.
	 */
	async subscribeToProject(projectId: string, event: Electron.IpcMainInvokeEvent) {
		const projectWatcher = await this.ensureProjectWatcher(projectId);
		this.registerSenderCleanup(event.sender);

		const subscriptionId = randomUUID();
		const eventChannel = `workspace:watcher-event:${randomUUID()}`;

		this.watcherSubscriptions.set(subscriptionId, {
			projectId,
			sender: event.sender,
			senderId: event.sender.id,
			eventChannel,
		});
		projectWatcher.subscriptionIds.add(subscriptionId);
		this.addSenderSubscription(event.sender.id, subscriptionId);

		return { subscriptionId, eventChannel };
	}

	/**
	 * Delete a subscription.
	 *
	 * If this is the last subscription to a project, its watchter is stopped and the handle is dropped.
	 */
	removeSubscription(subscriptionId: string): boolean {
		const subscription = this.watcherSubscriptions.get(subscriptionId);
		if (!subscription) return false;

		// Cleanup subscription state.
		this.watcherSubscriptions.delete(subscriptionId);

		// Remove this subscription from all the sender (window) subscriptions.
		// If this was the last subscription for this sender, we remove the sender from the map.
		const senderIds = this.senderSubscriptions.get(subscription.senderId);
		if (senderIds) {
			senderIds.delete(subscriptionId);
			if (senderIds.size === 0) this.senderSubscriptions.delete(subscription.senderId);
		}

		// Remove the project subscription.
		// If this was the last subscription to the project, the watcher is stopped.
		const projectWatcher = this.projectWatchers.get(subscription.projectId);
		if (projectWatcher) {
			projectWatcher.subscriptionIds.delete(subscriptionId);
			if (projectWatcher.subscriptionIds.size === 0) {
				try {
					projectWatcher.handle.stop();
				} catch (error) {
					// oxlint-disable-next-line no-console
					console.warn("Failed to stop project watcher", error);
				}
				this.projectWatchers.delete(subscription.projectId);
			}
		}

		return true;
	}

	private removeSenderSubscriptions(senderId: number): void {
		const subscriptionIds = this.senderSubscriptions.get(senderId);
		if (!subscriptionIds) return;

		for (const subscriptionId of subscriptionIds) this.removeSubscription(subscriptionId);

		this.senderSubscriptions.delete(senderId);
	}

	/**
	 * Sent the project watcher event to all the project subscribers.
	 *
	 * If there are any dead subscribers found, or if the messages fail to be sent,
	 * we'll remove the subscription.
	 * @param projectId - The project ID subscribed to.
	 * @param event - The watcher event being forwarded.
	 */
	private forwardWatcherEvent(projectId: string, event: WatcherEvent): void {
		const projectWatcher = this.projectWatchers.get(projectId);
		if (!projectWatcher) return;

		const deadSubscriptions: Array<string> = [];
		for (const subscriptionId of projectWatcher.subscriptionIds) {
			const subscription = this.watcherSubscriptions.get(subscriptionId);
			if (!subscription || subscription.sender.isDestroyed()) {
				// If there are any dead subscribers, i.e. windows that have been closed before
				// they unsubscribed, mark them as dead.
				deadSubscriptions.push(subscriptionId);
				continue;
			}

			try {
				// Forward the event to the right subscription channel.
				subscription.sender.send(subscription.eventChannel, event);
			} catch (sendError) {
				// oxlint-disable-next-line no-console
				console.warn("Failed to forward watcher event to renderer", sendError);
				deadSubscriptions.push(subscriptionId);
			}
		}

		for (const subscriptionId of deadSubscriptions) this.removeSubscription(subscriptionId);
	}

	/**
	 * Start a project watcher or return the existing one.
	 *
	 * This function is responsible for starting the watcher and ensuring that there's ever only one watcher per project.
	 */
	private async ensureProjectWatcher(projectId: string): Promise<ProjectWatcherState> {
		// There are previous subscriptions to the project and a watcher is already running.
		const existing = this.projectWatchers.get(projectId);
		if (existing) return existing;

		// There is already a watcher being started by a previous subscriber.
		//
		// This is needed for the case in which two subscribers want
		// to subscribe to the same project at the same time (or very close to each other).
		const pending = this.pendingProjectWatchers.get(projectId);
		if (pending) return pending;

		// Create a watcher.
		const creation = watcherStart(projectId, (err, event) => {
			if (err) {
				// oxlint-disable-next-line no-console
				console.warn("Watcher callback failed", err);
				return;
			}
			this.forwardWatcherEvent(projectId, event);
		})
			.then((handle) => {
				const watcherState: ProjectWatcherState = {
					handle,
					subscriptionIds: new Set(),
				};
				// Once the watcher has been started, store the handle in the state.
				this.projectWatchers.set(projectId, watcherState);
				return watcherState;
			})
			.finally(() => {
				// Once the creation has been fulfilled, remove it from the pending map.
				this.pendingProjectWatchers.delete(projectId);
			});

		// While the watcher is being created, store it in the pending watchers,
		// so that we can avoid creating multiple handles for the same project.
		this.pendingProjectWatchers.set(projectId, creation);
		return creation;
	}

	stopAllWatchersForShutdown(): number {
		const stopped = this.watcherSubscriptions.size;

		for (const [projectId, projectWatcher] of this.projectWatchers)
			try {
				projectWatcher.handle.stop();
			} catch (error) {
				// oxlint-disable-next-line no-console
				console.warn(`Failed to stop project watcher for ${projectId}`, error);
			}

		this.pendingProjectWatchers.clear();
		this.projectWatchers.clear();
		this.watcherSubscriptions.clear();
		this.senderSubscriptions.clear();

		return stopped;
	}

	/**
	 * Setup a listener that removes all subscriptions for a given sender on it's destruction.
	 *
	 * Here we ensure that on the closing of a window, all subscriptions associated are closed.
	 */
	private registerSenderCleanup(sender: Electron.WebContents): void {
		const senderId = sender.id;
		if (this.senderSubscriptions.has(senderId)) return;

		this.senderSubscriptions.set(senderId, new Set());
		sender.once("destroyed", () => {
			this.removeSenderSubscriptions(senderId);
		});
	}

	/**
	 * Store the subscriber to subscription relationship in the state.
	 */
	private addSenderSubscription(senderId: number, subscriptionId: string): void {
		const subscriptions = this.senderSubscriptions.get(senderId);
		if (!subscriptions) {
			this.senderSubscriptions.set(senderId, new Set([subscriptionId]));
			return;
		}

		subscriptions.add(subscriptionId);
	}

	/**
	 * Stop all watchers and destroy the instance of the watcher manager.
	 *
	 * This needs to be called on application shotdown.
	 */
	destroy(): void {
		try {
			this.stopAllWatchersForShutdown();
			WatcherManager.instance = null;
		} catch (error) {
			// oxlint-disable-next-line no-console
			console.warn("Failed to stop project watchers during shutdown", error);
		}
	}
}
