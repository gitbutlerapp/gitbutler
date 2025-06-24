import { invalidatesList, ReduxTag } from '$lib/state/tags';
import type { BackendApi, ClientState } from '$lib/state/clientState.svelte';

export class ActionService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get figureOutTheCommits() {
		return this.api.endpoints.idk.useMutation();
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			idk: build.mutation<void, { projectId: string }>({
				query: ({ projectId }) => ({
					command: 'idk',
					params: { projectId },
					actionName: 'Figure out the commits'
				}),
				invalidatesTags: [
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.WorktreeChanges)
				]
			})
		})
	});
}
