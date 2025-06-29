import type { ClientState } from '$lib/state/clientState.svelte';

export default class DbService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: ClientState['backendApi']) {
		this.api = injectEndpoints(backendApi);
	}

	startWatchingDb(projectId: string) {
		this.api.endpoints.startWatchingDb.mutate({ projectId });
		return () => {
			this.api.endpoints.stopWatchingDb.mutate({ projectId });
		};
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			startWatchingDb: build.mutation<string, { projectId: string }>({
				query: ({ projectId }) => ({
					command: 'start_watching_db',
					params: { projectId },
					actionName: 'Start watching the database for changes'
				}),
				invalidatesTags: [],
				transformResponse: (_res, _meta, args) => {
					return args.projectId;
				}
			}),
			stopWatchingDb: build.mutation<string, { projectId: string }>({
				query: ({ projectId }) => ({
					command: 'stop_watching_db',
					params: { projectId },
					actionName: 'Stop watching the database for changes'
				}),
				invalidatesTags: [],
				transformResponse: (_res, _meta, args) => {
					return args.projectId;
				}
			})
		})
	});
}
