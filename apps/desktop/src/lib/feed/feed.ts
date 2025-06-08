import { ActionListing, ButlerAction } from '$lib/actions/types';
import { invoke } from '$lib/backend/ipc';
import { Snapshot, type Operation } from '$lib/history/types';
import { plainToInstance } from 'class-transformer';
import { get, writable } from 'svelte/store';

export class Feed {
	private oplogBuffer: Snapshot[] = [];
	private oplogCursor: string | undefined = undefined;
	private actionsBuffer: ButlerAction[] = [];

	readonly combined = writable<(Snapshot | ButlerAction)[]>([], (set) => {
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

	async fetch(n: number = 20) {
		// First, get n oplog entries and n actions. They are already sorted by timestamp.
		// If the oplog buffer hass less than n entries, we need to fetch more oplog entries.
		if (this.oplogBuffer.length < n) {
			const moreOplog = await this.fetchOplog(n - this.oplogBuffer.length, this.oplogCursor);
			this.oplogCursor = moreOplog.at(-1)?.id;
			this.oplogBuffer.push(...moreOplog);
		}

		// If the actions buffer has less than n entries, we need to fetch more actions.
		if (this.actionsBuffer.length < n) {
			const moreActions = await this.fetchActions(
				n - this.actionsBuffer.length,
				this.actionsOffset()
			);
			this.actionsBuffer.push(...moreActions);
		}

		// Then, create a combined feed list with n entries, maintaining the sorting by time and consuming items from the buffers.
		// Since the combined feed has the same n, not all entries will be consumed from both buffers.
		for (let i = 0; i < n; i++) {
			if (this.oplogBuffer.length === 0 && this.actionsBuffer.length === 0) {
				break; // No more entries to consume.
			}
			const firstOplog = this.oplogBuffer[0];
			const firstAction = this.actionsBuffer[0];
			if (firstOplog && firstAction) {
				if (firstOplog.createdAt >= firstAction.createdAt) {
					this.combined.update((entries) => [...entries, firstOplog]);
					this.oplogBuffer.shift(); // Consume the oplog entry.
				} else {
					this.combined.update((entries) => [...entries, firstAction]);
					this.actionsBuffer.shift(); // Consume the action entry.
				}
			} else if (firstOplog) {
				this.combined.update((entries) => [...entries, firstOplog]);
				this.oplogBuffer.shift(); // Consume the oplog entry.
			} else if (firstAction) {
				this.combined.update((entries) => [...entries, firstAction]);
				this.actionsBuffer.shift(); // Consume the action entry.
			}
		}
	}

	private async fetchOplog(count: number, after?: string) {
		const exclude: Operation[] = [
			'AutoHandleChangesAfter',
			'AutoHandleChangesBefore',
			'FileChanges'
		];
		const resp = await invoke<Snapshot[]>('list_snapshots', {
			projectId: this.projectId,
			sha: after,
			limit: count,
			excludeKind: exclude
		});
		const snapshots = plainToInstance(Snapshot, resp);
		return snapshots;
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
}
