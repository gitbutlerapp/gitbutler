import type { ActionListing } from '$lib/actions/types';
import type { ClientState } from '$lib/state/clientState.svelte';

export default class ActionService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: ClientState['backendApi']) {
		this.api = injectEndpoints(backendApi);
	}

	listActions(projectId: string, page: number = 1, pageSize: number = 10) {
		return this.api.endpoints.listActions.useQuery({ projectId, page, pageSize });
	}

	get revertSnapshot() {
		return this.api.endpoints.actionsRevertSnapshot.useMutation();
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listActions: build.query<
				ActionListing,
				{ projectId: string; page: number; pageSize: number }
			>({
				query: ({ projectId, page, pageSize }) => ({
					command: 'list_actions',
					params: { projectId, page, pageSize }
				})
			}),
			actionsRevertSnapshot: build.mutation<
				void,
				{ projectId: string; snapshot: string; description: string }
			>({
				query: ({ projectId, snapshot, description }) => ({
					command: 'actions_revert_snapshot',
					params: { projectId, snapshot, description }
				})
			})
		})
	});
}
