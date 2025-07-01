import { ActionListing, ButlerAction, Workflow, WorkflowList } from '$lib/actions/types';
import { invoke } from '$lib/backend/ipc';
import { Snapshot } from '$lib/history/types';
import { plainToInstance } from 'class-transformer';
import { get, writable } from 'svelte/store';

export class Feed {
	private actionsBuffer: ButlerAction[] = [];
	private workflowsBuffer: Workflow[] = [];

	readonly combined = writable<(Snapshot | ButlerAction | Workflow)[]>([], (set) => {
		this.fetch();
		return () => {
			set([]);
		};
	});

	constructor(private projectId: string) {}

	// The offset is equal to the number of entries in the actions buffer plus the number of ButlerAction entries in the combined feed.
	private actionsOffset(): number {
		return (
			this.actionsBuffer.length +
			get(this.combined).filter((entry) => entry instanceof ButlerAction).length
		);
	}

	private workflowsOffset(): number {
		return (
			this.workflowsBuffer.length +
			get(this.combined).filter((entry) => entry instanceof Workflow).length
		);
	}

	async fetch(n: number = 20) {
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
				this.combined.update((entries) => [...entries, mostRecent]);
				// Shift the corresponding buffer, based on the type of the earliest entry.
				if (mostRecent instanceof ButlerAction) {
					this.actionsBuffer.shift(); // Consume the action entry.
				} else if (mostRecent instanceof Workflow) {
					this.workflowsBuffer.shift(); // Consume the workflow entry.
				}
			}
		}
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
}
