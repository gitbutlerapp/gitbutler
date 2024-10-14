import { Snapshot, SnapshotDiff } from './types';
import { invoke } from '$lib/backend/ipc';
import { plainToInstance } from 'class-transformer';
import { get, writable } from 'svelte/store';

export class HistoryService {
	cursor: string | undefined = undefined;

	readonly loading = writable(false);
	readonly isAllLoaded = writable(false);
	readonly snapshots = writable<Snapshot[]>([], (set) => {
		// Load snapshots when going from 0 -> 1 subscriber.
		this.load();
		return () => {
			// Clear store when component last subscriber unsubscribes.
			set([]);
			this.cursor = undefined;
			this.isAllLoaded.set(false);
		};
	});

	constructor(private projectId: string) {}

	async load() {
		const data = await this.fetch();
		if (data.length) {
			this.snapshots.set(data);
			this.cursor = data.at(-1)?.id;
		}
	}

	async loadMore() {
		if (!this.cursor) throw new Error('Not without a cursor');
		if (get(this.isAllLoaded)) return; // Nothing to do.

		// TODO: Update API so we don't have to .slice()
		const more = (await this.fetch(this.cursor)).slice(1);

		if (more.length === 0) {
			this.isAllLoaded.set(true);
		} else {
			this.snapshots.update((snapshots) => [...snapshots, ...more]);
			this.cursor = more.at(-1)?.id;
		}
	}

	private async fetch(after?: string) {
		this.loading.set(true);
		const resp = await invoke<Snapshot[]>('list_snapshots', {
			projectId: this.projectId,
			sha: after,
			limit: 32
		});
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
	return `${t.toDateString() === d.toDateString() ? 'Today' : d.toLocaleDateString('en-US', { weekday: 'short' })}, ${d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}`;
}
