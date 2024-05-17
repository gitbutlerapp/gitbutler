import { Snapshot, SnapshotDiff } from './types';
import { invoke } from '$lib/backend/ipc';
import { plainToInstance } from 'class-transformer';
import { writable } from 'svelte/store';

export class HistoryService {
	cursor: string | undefined = undefined;

	snapshots = writable<Snapshot[]>([], (set) => {
		this.loadSnapshots().then((x) => set(x));
		return () => {
			set([]);
			this.cursor = undefined;
		};
	});
	loading = writable(false);

	constructor(private projectId: string) {}

	async loadSnapshots(after?: string) {
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

	async loadMore() {
		if (!this.cursor) throw new Error('Unable to load more without a cursor');
		const more = await this.loadSnapshots(this.cursor);
		this.snapshots.update((snapshots) => [...snapshots, ...more.slice(1)]);
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
