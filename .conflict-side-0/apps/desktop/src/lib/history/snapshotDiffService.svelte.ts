import { SnapshotDiff } from '$lib/history/types';
import { providesItem, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/shared/context';
import { plainToInstance } from 'class-transformer';
import type { BackendApi, ClientState } from '$lib/state/clientState.svelte';

export const SNAPSHOT_DIFF_SERVICE = new InjectionToken<SnapshotDiffService>('SnapshotDiffService');

export default class SnapshotDiffService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	getDiff(projectId: string, sha: string) {
		return this.api.endpoints.diff.useQuery(
			{ projectId, sha },
			{
				transform: (response: { [key: string]: unknown }) => {
					return Object.entries(response).reduce<{ [key: string]: SnapshotDiff }>(
						(acc, [path, diff]) => {
							acc[path] = plainToInstance(SnapshotDiff, diff);
							return acc;
						},
						{}
					);
				}
			}
		);
	}

	getFileDiffs(projectId: string, sha: string) {
		return this.api.endpoints.diff.useQuery(
			{ projectId, sha },
			{
				transform: (response: { [key: string]: unknown }) => Object.keys(response)
			}
		);
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			diff: build.query<{ [key: string]: unknown }, { projectId: string; sha: string }>({
				extraOptions: {
					command: 'snapshot_diff'
				},
				query: (args) => args,
				providesTags: (_result, _error, args) => providesItem(ReduxTag.SnapshotDiff, args.sha)
			})
		})
	});
}
