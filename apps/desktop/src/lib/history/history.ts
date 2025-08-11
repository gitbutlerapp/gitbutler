import { invoke } from '$lib/backend/ipc';
import { Snapshot } from '$lib/history/types';
import { InjectionToken } from '@gitbutler/shared/context';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import { plainToInstance } from 'class-transformer';
import { get, writable } from 'svelte/store';
import type { TreeChange } from '$lib/hunks/change';
import type { BackendApi, ClientState } from '$lib/state/clientState.svelte';

const snapshotDiffAdapter = createEntityAdapter({
	selectId: (tc: TreeChange) => tc.path
});
const snapshotDiffSelectors = snapshotDiffAdapter.getSelectors();

export const HISTORY_SERVICE = new InjectionToken<HistoryService>('HistoryService');
class SnapshotPager {
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

	constructor(private readonly projectId: string) {}

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
}

type SnapshotDiffParams = {
	projectId: string;
	snapshotId: string;
};

export class HistoryService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	#snapshots = new Map<string, SnapshotPager>();
	snapshots(projectId: string) {
		let snapshot = this.#snapshots.get(projectId);
		if (!snapshot) {
			snapshot = new SnapshotPager(projectId);
			this.#snapshots.set(projectId, snapshot);
		}
		return snapshot;
	}

	async getSnapshotDiff(projectId: string, snapshotId: string): Promise<TreeChange[]> {
		return await this.api.endpoints.snapshotDiff.fetch(
			{ projectId, snapshotId },
			{ transform: snapshotDiffSelectors.selectAll }
		);
	}

	snapshotDiff(params: SnapshotDiffParams) {
		return this.api.endpoints.snapshotDiff.useQuery(params, {
			transform: snapshotDiffSelectors.selectAll
		});
	}

	snapshotDiffByPath(params: SnapshotDiffParams & { path: string }) {
		return this.api.endpoints.snapshotDiff.useQuery(
			{ projectId: params.projectId, snapshotId: params.snapshotId },
			{
				transform: (data) => snapshotDiffSelectors.selectById(data, params.path)
			}
		);
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

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			snapshotDiff: build.query<EntityState<TreeChange, string>, SnapshotDiffParams>({
				extraOptions: { command: 'snapshot_diff' },
				query: ({ projectId, snapshotId }) => ({ projectId, sha: snapshotId }),
				transformResponse: (data: TreeChange[]) => {
					return snapshotDiffAdapter.addMany(snapshotDiffAdapter.getInitialState(), data);
				}
			})
		})
	});
}
