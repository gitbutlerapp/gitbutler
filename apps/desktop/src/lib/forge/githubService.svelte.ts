import { ReduxTag } from '$lib/state/tags';
import type { TreeChange } from '$lib/hunks/change';
import type { ClientState } from '$lib/state/clientState.svelte';
import type { PullRequest } from './interface/types';
import type { EntityState } from '@reduxjs/toolkit';

export class WorktreeService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.githubApi);
	}
}

function injectEndpoints(api: ClientState['githubApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getChanges: build.query<EntityState<PullRequest, string>, { projectId: string }>({
				query: ({ projectId }) => ({ action: 'pulls', method: 'get', params: { projectId } }),
				providesTags: [ReduxTag.PullRequests]
			})
		})
	});
}
