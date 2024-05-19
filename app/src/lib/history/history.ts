import { Snapshot, SnapshotDiff } from './types';
import { invoke } from '$lib/backend/ipc';
import { plainToInstance } from 'class-transformer';
import { writable } from 'svelte/store';

export class HistoryService {
	cursor: string | undefined = undefined;

	readonly loading = writable(false);
	readonly snapshots = writable<Snapshot[]>([], (set) => {
		// Load snapshots when going from 0 -> 1 subscriber.
		this.fetch().then((x) => set(x));
		return () => {
			// Clear store when component last subscriber unsubscribes.
			set([]);
			this.cursor = undefined;
		};
	});

	constructor(private projectId: string) {}

	async load() {
		if (this.cursor) this.cursor = undefined;
		this.snapshots.set(await this.fetch());
		this.loading.set(false);
	}

	async loadMore() {
		if (!this.cursor) throw new Error('Unable to load more without a cursor');
		const more = await this.fetch(this.cursor);
		// TODO: Update API so we don't have to .slice()
		this.snapshots.update((snapshots) => [...snapshots, ...more.slice(1)]);
	}

	private async fetch(after?: string) {
		this.loading.set(true);
		const resp = await invoke<Snapshot[]>('list_snapshots', {
			projectId: this.projectId,
			sha: after,
			limit: 32
		});
		this.cursor = resp.length > 0 ? resp[resp.length - 1].id : undefined;
		this.loading.set(false);
		return plainToInstance(Snapshot, resp);
	}

	clear() {
		this.snapshots.set([]);
	}

	async getSnapshotDiff(projectId: string, sha: string) {
		const resp = await invoke<{ [key: string]: any }>('snapshot_diff', {
			projectId: projectId,
			sha: sha
		});
		return Object.entries(resp).reduce<{ [key: string]: SnapshotDiff }>((acc, [path, diff]) => {
			acc[path] = plainToInstance(SnapshotDiff, diff);
			return acc;
		}, {});
	}

	async restoreSnapshot(projectId: string, sha: string) {
		await invoke<string>('restore_snapshot', {
			projectId: projectId,
			sha: sha
		});
	}
}

export function createdOnDay(d: Date) {
	const t = new Date();
	return `${t.toDateString() == d.toDateString() ? 'Today' : d.toLocaleDateString('en-US', { weekday: 'short' })}, ${d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}`;
}
