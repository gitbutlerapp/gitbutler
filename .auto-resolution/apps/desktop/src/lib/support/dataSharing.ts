import { InjectionToken } from '@gitbutler/shared/context';
import type { BackendApi } from '$lib/state/clientState.svelte';

export const DATA_SHARING_SERVICE = new InjectionToken<DataSharingService>('DataSharingService');

export default class DataSharingService {
	private api: ReturnType<typeof injectEndpoints>;
	constructor(private backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	async logs() {
		return await this.api.endpoints.logs.fetch(undefined, { forceRefetch: true });
	}

	async projectData(projectId: string) {
		return await this.api.endpoints.projectData.fetch({ projectId }, { forceRefetch: true });
	}
}

function injectEndpoints(backendApi: BackendApi) {
	return backendApi.injectEndpoints({
		endpoints: (build) => ({
			logs: build.query<string, undefined>({
				extraOptions: { command: 'get_logs_archive_path' },
				query: () => ({})
			}),
			projectData: build.query<string, { projectId: string }>({
				extraOptions: { command: 'get_project_archive_path' },
				query: (params) => params
			})
		})
	});
}
