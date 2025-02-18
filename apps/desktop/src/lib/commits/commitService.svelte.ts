import { Commit } from './commit';
import { ReduxTag } from '$lib/state/tags';
import { plainToInstance } from 'class-transformer';
import type { ClientState } from '$lib/state/clientState.svelte';

export class CommitService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.backendApi);
	}

	find(projectId: string, commitOid: string) {
		const result = $derived(this.api.endpoints.find.useQuery({ projectId, commitOid }));
		return result;
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			find: build.query<Commit, { projectId: string; commitOid: string }>({
				query: ({ projectId, commitOid }) => ({
					command: 'find_commit',
					params: { projectId, commitOid }
				}),
				transformResponse: (response: unknown) => plainToInstance(Commit, response),
				providesTags: [ReduxTag.Commit]
			})
		})
	});
}
