import { ActionListing, ButlerAction, Workflow, WorkflowList } from '$lib/actions/types';
import { Snapshot } from '$lib/history/types';
import Mutex from '$lib/utils/mutex';
import { InjectionToken } from '@gitbutler/shared/context';
import { deduplicateBy } from '@gitbutler/shared/utils/array';
import { plainToInstance } from 'class-transformer';
import { get, writable } from 'svelte/store';
import type { ToolCall } from '$lib/ai/tool';
import type { IBackend } from '$lib/backend';
import type { StackService } from '$lib/stacks/stackService.svelte';

export const FEED_FACTORY = new InjectionToken<FeedFactory>('FeedFactory');

export default class FeedFactory {
	private instance: Feed | null = null;

	constructor(
		private backend: IBackend,
		private stackService: StackService
	) {}

	/**
	 * Gets or creates a Feed instance for the given project ID.
	 *
	 * If an instance already exists for the project ID, it returns that instance
	 *
	 * If the instance exists but is for a different project ID, it unsubscribes from the previous instance
	 * and creates a new instance for the new project ID.
	 */
	getFeed(projectId: string): Feed {
		if (!this.instance) {
			this.instance = new Feed(this.backend, projectId, this.stackService);
		}

		if (!this.instance.isProjectFeed(projectId)) {
			this.instance.unlisten();
			this.instance = new Feed(this.backend, projectId, this.stackService);
		}

		return this.instance;
	}
}

type DBEvent = {
	kind: 'actions' | 'workflows' | 'unknown';
	item?: string;
};

type TokenEvent = {
	messageId: string;
	token: string;
};

type ToolCallEvent = ToolCall & {
	messageId: string;
};

export type TodoState = {
	id: string;
	title: string;
	status: 'waiting' | 'in-progress' | 'success' | 'failed';
};

type TodoUpdateEvent = {
	messageId: string;
	list: TodoState[];
};

type UserMessageId = `user-${string}`;

export type UserMessage = {
	id: UserMessageId;
	type: 'user';
	content: string;
};

type AssistantMessageId = `assistant-${string}`;

export type AssistantMessage = {
	id: AssistantMessageId;
	type: 'assistant';
	content: string;
	toolCalls: ToolCall[];
};

type InProgressAssistantMessageId = `assistant-in-progress-${string}`;

export type InProgressAssistantMessage = {
	id: InProgressAssistantMessageId;
	type: 'assistant-in-progress';
	content: string;
	toolCalls: ToolCall[];
	todos: TodoState[];
};

export type FeedMessage = UserMessage | AssistantMessage;

export type FeedEntry =
	| ButlerAction
	| Workflow
	| Snapshot
	| FeedMessage
	| InProgressAssistantMessage;

export function isFeedMessage(entry: FeedEntry): entry is FeedMessage {
	return (entry as FeedMessage).type === 'user' || (entry as FeedMessage).type === 'assistant';
}

export function isInProgressAssistantMessage(
	entry: FeedEntry
): entry is InProgressAssistantMessage {
	return (entry as InProgressAssistantMessage).id.startsWith('assistant-in-progress-');
}

interface BaseInProgressUpdate {
	type: 'token' | 'tool-call' | 'todo-update';
}

export interface TokenUpdate extends BaseInProgressUpdate {
	type: 'token';
	token: string;
}

export interface ToolCallUpdate extends BaseInProgressUpdate {
	type: 'tool-call';
	toolCall: ToolCall;
}

export interface TodoUpdate extends BaseInProgressUpdate {
	type: 'todo-update';
	list: TodoState[];
}

export type InProgressUpdate = TokenUpdate | ToolCallUpdate | TodoUpdate;

type InProgressSubscribeCallback = (update: InProgressUpdate) => void;

class Feed {
	private actionsBuffer: ButlerAction[];
	private workflowsBuffer: Workflow[];
	private unlistenDB: () => void;
	private unlistenTokens: () => void;
	private unlistenToolCalls: () => void;
	private unlistenTodoUpdates: () => void;
	private initialized;
	private mutex = new Mutex();
	private updateTimeout: ReturnType<typeof setTimeout> | null = null;
	private messageSubscribers: Map<InProgressAssistantMessageId, InProgressSubscribeCallback[]>;

	readonly lastAddedId = writable<string | null>(null);

	readonly combined = writable<FeedEntry[]>([], () => {
		this.fetch();
	});

	constructor(
		private backend: IBackend,
		private projectId: string,
		private stackService: StackService
	) {
		this.actionsBuffer = [];
		this.workflowsBuffer = [];
		this.messageSubscribers = new Map();
		this.initialized = false;

		this.unlistenDB = this.backend.listen<DBEvent>(`project://${projectId}/db-updates`, (event) => {
			this.handleDBEvent(event.payload);
		});

		this.unlistenTokens = this.backend.listen<TokenEvent>(
			`project://${projectId}/token-updates`,
			(event) => {
				this.handleTokenEvent(event.payload);
			}
		);

		this.unlistenToolCalls = this.backend.listen<ToolCallEvent>(
			`project://${projectId}/tool-call`,
			(event) => {
				this.handleToolCallEvent(event.payload);
			}
		);

		this.unlistenTodoUpdates = this.backend.listen<TodoUpdateEvent>(
			`project://${projectId}/todo-updates`,
			(event) => {
				this.handleTodoUpdate(event.payload);
			}
		);
	}

	isProjectFeed(projectId: string): boolean {
		return this.projectId === projectId;
	}

	subscribeToMessage(
		messageId: InProgressAssistantMessageId,
		callback: InProgressSubscribeCallback
	): () => void {
		if (!this.messageSubscribers.has(messageId)) {
			this.messageSubscribers.set(messageId, []);
		}

		const subscribers = this.messageSubscribers.get(messageId) ?? [];
		subscribers.push(callback);
		this.messageSubscribers.set(messageId, subscribers);

		return () => {
			const subscribers = this.messageSubscribers.get(messageId) ?? [];
			const index = subscribers.indexOf(callback);
			if (index !== -1) {
				subscribers.splice(index, 1);
				this.messageSubscribers.set(messageId, subscribers);
			}
		};
	}

	private notifySubscribersToken(messageId: InProgressAssistantMessageId, token: string) {
		const subscribers = this.messageSubscribers.get(messageId) ?? [];
		subscribers.forEach((callback) => callback({ type: 'token', token }));
	}

	private notifySubscribersToolCall(messageId: InProgressAssistantMessageId, toolCall: ToolCall) {
		const subscribers = this.messageSubscribers.get(messageId) ?? [];
		subscribers.forEach((callback) => callback({ type: 'tool-call', toolCall }));
	}

	private notifySubscribersTodoUpdate(messageId: InProgressAssistantMessageId, list: TodoState[]) {
		const subscribers = this.messageSubscribers.get(messageId) ?? [];
		subscribers.forEach((callback) => callback({ type: 'todo-update', list }));
	}

	private handleDBEvent(event: DBEvent) {
		switch (event.kind) {
			case 'actions':
			case 'workflows': {
				this.updateCombinedFeed();
				return;
			}
			case 'unknown': {
				// Do nothing for now, as we are not handling these events.
				return;
			}
		}
	}

	private async handleTokenEvent(event: TokenEvent) {
		const inProgressId: InProgressAssistantMessageId = `assistant-in-progress-${event.messageId}`;
		this.notifySubscribersToken(inProgressId, event.token);
	}

	private async handleToolCallEvent(event: ToolCallEvent) {
		const { messageId, name, parameters, result } = event;
		const inProgressId: InProgressAssistantMessageId = `assistant-in-progress-${messageId}`;
		this.notifySubscribersToolCall(inProgressId, { name, parameters, result });

		await this.mutex.lock(async () => {
			this.combined.update((entries) => {
				const existing = entries.find((entry) => entry.id === inProgressId);
				if (existing && isInProgressAssistantMessage(existing)) {
					existing.toolCalls.push({ name, parameters, result });
					return deduplicateBy([...entries], 'id');
				}
				return entries; // If not found, do nothing.
			});
		});
	}

	private async handleTodoUpdate(event: TodoUpdateEvent) {
		const inProgressId: InProgressAssistantMessageId = `assistant-in-progress-${event.messageId}`;
		this.notifySubscribersTodoUpdate(inProgressId, event.list);

		await this.mutex.lock(async () => {
			this.combined.update((entries) => {
				const existing = entries.find((entry) => entry.id === inProgressId);
				if (existing && isInProgressAssistantMessage(existing)) {
					existing.todos = event.list;
					return deduplicateBy([...entries], 'id');
				}
				return entries; // If not found, do nothing.
			});
		});
	}

	private async handleLastAdded(entry: FeedEntry) {
		this.lastAddedId.set(entry.id);
	}

	private handleNewItemObserved(entry: FeedEntry) {
		if (entry instanceof Workflow) {
			const stackId = entry.kind.subject?.stackId;
			if (stackId) this.stackService.invalidateStackDetailsUpdate(stackId);
		}
	}

	private getFeedMessages(): FeedMessage[] {
		const messages = get(this.combined).filter((entry) => isFeedMessage(entry)) as FeedMessage[];
		return messages.reverse();
	}

	async addUserMessage(content: string): Promise<[string, FeedMessage[]]> {
		const uuid = crypto.randomUUID();
		const message: UserMessage = {
			id: `user-${uuid}` as UserMessageId,
			type: 'user',
			content
		};

		const inProgress: InProgressAssistantMessage = {
			id: `assistant-in-progress-${uuid}`,
			type: 'assistant-in-progress',
			content: '',
			toolCalls: [],
			todos: []
		};

		let added = false;
		await this.mutex.lock(async () => {
			this.combined.update((entries) => {
				const existing = entries.find((entry) => entry.id === message.id);
				if (!existing) {
					added = true;
					return [inProgress, message, ...entries];
				}
				return entries;
			});
		});

		if (added) this.handleLastAdded(inProgress);
		const messages = this.getFeedMessages();
		return [uuid, messages];
	}

	private getToolCallsAndRemoveInProgress(
		entries: FeedEntry[],
		inProgressId: InProgressAssistantMessageId
	): [ToolCall[], FeedEntry[]] {
		const toolCalls: ToolCall[] = [];
		const updatedEntries = entries.filter((entry) => {
			if (isInProgressAssistantMessage(entry) && entry.id === inProgressId) {
				toolCalls.push(...entry.toolCalls);
				return false; // Remove the in-progress message.
			}
			return true; // Keep other entries.
		});

		return [toolCalls, updatedEntries] as const;
	}

	async addAssistantMessage(uuid: string, content: string): Promise<AssistantMessage> {
		const inProgressId: InProgressAssistantMessageId = `assistant-in-progress-${uuid}`;
		const id: AssistantMessageId = `assistant-${uuid}`;
		const message: AssistantMessage = {
			id,
			type: 'assistant',
			content,
			toolCalls: []
		};

		let added = false;

		await this.mutex.lock(async () => {
			this.combined.update((entries) => {
				// Remove the in-progress message if it exists.
				const [toolCalls, updatedEntries] = this.getToolCallsAndRemoveInProgress(
					entries,
					inProgressId
				);

				message.toolCalls = toolCalls;

				const existing = updatedEntries.find((entry) => entry.id === id);
				if (!existing) {
					added = true;
					return [message, ...updatedEntries];
				}
				return updatedEntries;
			});
		});

		if (added) this.handleLastAdded(message);

		return message;
	}

	async updateCombinedFeed() {
		if (this.updateTimeout) {
			clearTimeout(this.updateTimeout);
		}

		this.updateTimeout = setTimeout(async () => {
			let lastAddedItem: FeedEntry | undefined = undefined;
			await this.mutex.lock(async () => {
				const n = 5;
				// If the actions buffer has less than n entries, we need to fetch more actions.
				const moreActions = await this.fetchActions(n, 0);
				moreActions.reverse();
				// If the workflows buffer has less than n entries, we need to fetch more workflows.
				const moreWorkflows = await this.fetchWorkflows(n, 0);
				moreWorkflows.reverse();

				// Then, create a combined feed list with n entries, maintaining the reverse sorting by time and consuming items from the buffers.
				// Since the combined feed has the same n, not all entries will be consumed from both buffers.
				while (moreActions.length > 0 || moreWorkflows.length > 0) {
					if (moreActions.length === 0 && moreWorkflows.length === 0) {
						break; // No more entries to consume.
					}
					const lastAction = moreActions[0];
					const lastWorkflow = moreWorkflows[0];

					const lessRecent = [lastAction, lastWorkflow]
						.filter((item) => item !== undefined)
						.sort((a, b) => a.createdAt.getTime() - b.createdAt.getTime())[0];

					if (lessRecent) {
						this.combined.update((entries) => {
							const existing = entries.find((entry) => entry.id === lessRecent.id);
							if (!existing) {
								lastAddedItem = lessRecent;

								this.handleNewItemObserved(lastAddedItem);

								if (!this.initialized) {
									return entries;
								}
								return [lessRecent, ...entries];
							}
							return entries;
						});
						// Shift the corresponding buffer, based on the type of the earliest entry.
						if (lessRecent instanceof ButlerAction) {
							moreActions.shift(); // Consume the action entry.
						} else if (lessRecent instanceof Workflow) {
							moreWorkflows.shift(); // Consume the workflow entry.
						}
					}
				}
			});

			if (lastAddedItem) {
				this.handleLastAdded(lastAddedItem);
			}
		}, 500);
	}

	// The offset is equal to the number of entries in the actions buffer plus the number of ButlerAction entries in the combined feed.
	private actionsOffset(prepend: boolean = false): number {
		if (prepend) {
			return 0;
		}

		return (
			this.actionsBuffer.length +
			get(this.combined).filter((entry) => entry instanceof ButlerAction).length
		);
	}

	private workflowsOffset(prepend: boolean = false): number {
		if (prepend) {
			return 0;
		}

		return (
			this.workflowsBuffer.length +
			get(this.combined).filter((entry) => entry instanceof Workflow).length
		);
	}

	private async fetchInner(n: number = 20) {
		// If the actions buffer has less than n entries, we need to fetch more actions.
		if (this.actionsBuffer.length < n) {
			const moreActions = await this.fetchActions(
				n - this.actionsBuffer.length,
				this.actionsOffset()
			);

			this.actionsBuffer.push(...moreActions);
		}
		// If the workflows buffer has less than n entries, we need to fetch more workflows.
		if (this.workflowsBuffer.length < n) {
			const moreWorkflows = await this.fetchWorkflows(
				n - this.workflowsBuffer.length,
				this.workflowsOffset()
			);
			this.workflowsBuffer.push(...moreWorkflows);
		}

		// Then, create a combined feed list with n entries, maintaining the sorting by time and consuming items from the buffers.
		// Since the combined feed has the same n, not all entries will be consumed from both buffers.
		for (let i = 0; i < n; i++) {
			if (this.actionsBuffer.length === 0 && this.workflowsBuffer.length === 0) {
				break; // No more entries to consume.
			}
			const firstAction = this.actionsBuffer[0];
			const firstWorkflow = this.workflowsBuffer[0];

			const mostRecent = [firstAction, firstWorkflow]
				.filter((item) => item !== undefined)
				.sort((a, b) => b.createdAt.getTime() - a.createdAt.getTime())[0];

			if (mostRecent) {
				this.combined.update((entries) => deduplicateBy([...entries, mostRecent], 'id'));
				// Shift the corresponding buffer, based on the type of the earliest entry.
				if (mostRecent instanceof ButlerAction) {
					this.actionsBuffer.shift(); // Consume the action entry.
				} else if (mostRecent instanceof Workflow) {
					this.workflowsBuffer.shift(); // Consume the workflow entry.
				}
			}
		}
	}

	async fetch(n: number = 20) {
		await this.mutex.lock(async () => {
			this.fetchInner(n);
		});
		this.initialized = true;
	}

	private async fetchActions(count: number, offset: number) {
		const listing = await this.backend.invoke<any>('list_actions', {
			projectId: this.projectId,
			offset: offset,
			limit: count
		});
		const actions = plainToInstance(ActionListing, listing).actions;
		return actions;
	}

	private async fetchWorkflows(count: number, offset: number) {
		const listing = await this.backend.invoke<any>('list_workflows', {
			projectId: this.projectId,
			offset: offset,
			limit: count
		});
		const workflows = plainToInstance(WorkflowList, listing).workflows;
		return workflows;
	}

	unlisten() {
		this.unlistenDB();
		this.unlistenTokens();
		this.unlistenToolCalls();
		this.unlistenTodoUpdates();
	}
}
