import { ActionListing, ButlerAction, Workflow, WorkflowList } from '$lib/actions/types';
import { invoke } from '$lib/backend/ipc';
import { Snapshot } from '$lib/history/types';
import { deduplicateBy } from '@gitbutler/shared/utils/array';
import { plainToInstance } from 'class-transformer';
import { get, writable } from 'svelte/store';
import type { Tauri } from '$lib/backend/tauri';

type DBEvent = {
	kind: 'actions' | 'workflows' | 'hunk-assignments' | 'unknown';
	item?: string;
};

class Mutex {
	private mutex = Promise.resolve();

	async lock<T>(fn: () => Promise<T>): Promise<T> {
		let release: () => void;
		const wait = new Promise<void>((res) => (release = res));
		const prev = this.mutex;
		this.mutex = prev.then(async () => await wait);
		await prev;
		try {
			return await fn();
		} finally {
			release!();
		}
	}
}

export class Feed {
	private actionsBuffer: ButlerAction[] = [];
	private workflowsBuffer: Workflow[] = [];
	private unlistenDB: () => void;
	private initialized = false;
	private mutex = new Mutex();
	private updateTimeout: ReturnType<typeof setTimeout> | null = null;

	readonly lastAddedId = writable<string | null>(null);
	readonly stackToUpdate = writable<string | null>(null);

	readonly combined = writable<(Snapshot | ButlerAction | Workflow)[]>([], () => {
		this.fetch();
	});

	constructor(
		private tauri: Tauri,
		private projectId: string
	) {
		this.unlistenDB = this.tauri.listen<DBEvent>(`project://${projectId}/db-updates`, (event) => {
			this.handleDBEvent(event.payload);
		});
	}

	private handleDBEvent(event: DBEvent) {
		switch (event.kind) {
			case 'actions':
			case 'workflows': {
				this.updateCombinedFeed();
				return;
			}
			case 'hunk-assignments':
			case 'unknown': {
				// Do nothing for now, as we are not handling these events.
				return;
			}
		}
	}

	private async handleLastAdded(entry: Snapshot | ButlerAction | Workflow) {
		this.lastAddedId.set(entry.id);

		if (entry instanceof Workflow) {
			const stackId = entry.kind.subject?.stackId;
			if (stackId) this.stackToUpdate.set(stackId);
		}
	}

	async updateCombinedFeed() {
		if (!this.initialized) return;
		if (this.updateTimeout) {
			clearTimeout(this.updateTimeout);
		}

		this.updateTimeout = setTimeout(async () => {
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
								this.handleLastAdded(lessRecent);
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
		const listing = await invoke<any>('list_actions', {
			projectId: this.projectId,
			offset: offset,
			limit: count
		});
		const actions = plainToInstance(ActionListing, listing).actions;
		return actions;
	}

	private async fetchWorkflows(count: number, offset: number) {
		const listing = await invoke<any>('list_workflows', {
			projectId: this.projectId,
			offset: offset,
			limit: count
		});
		const workflows = plainToInstance(WorkflowList, listing).workflows;
		return workflows;
	}

	unlisten() {
		this.unlistenDB();
	}
}
