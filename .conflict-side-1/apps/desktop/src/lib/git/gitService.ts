import { providesList, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/shared/context';
import type { IBackend } from '$lib/backend';
import type { BackendApi } from '$lib/state/clientState.svelte';

export const GIT_SERVICE = new InjectionToken<GitService>('GitService');

export class GitService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		private backend: IBackend,
		backendApi: BackendApi
	) {
		this.api = injectEndpoints(backendApi);
	}

	/**
	 * Emits a new value when a fetch was detected by the back end.
	 * @example
	 * $effect(() => gitService.onFetch(data.projectId, () => {}));
	 */
	onFetch(projectId: string, callback: () => void) {
		return this.backend.listen<any>(`project://${projectId}/git/fetch`, callback);
	}

	async checkSigningSettings(projectId: string): Promise<void> {
		return await this.backend.invoke('check_signing_settings', { projectId });
	}

	async indexSize(projectId: string): Promise<number> {
		return await this.backend.invoke('git_index_size', { projectId });
	}

	async cloneRepo(repoUrl: string, dir: string): Promise<void> {
		await this.backend.invoke('git_clone_repository', {
			repositoryUrl: repoUrl,
			targetDir: dir
		});
	}

	getAuthorInfo(projectId: string) {
		return this.api.endpoints.authorInfo.useQuery({ projectId });
	}

	get setAuthorInfo() {
		return this.api.endpoints.setAuthorInfo.useMutation();
	}
}

function injectEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			authorInfo: build.query<AuthorInfo, { projectId: string }>({
				extraOptions: { command: 'get_author_info' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.AuthorInfo)]
			}),
			setAuthorInfo: build.mutation<void, { projectId: string; name: string; email: string }>({
				extraOptions: { command: 'store_author_globally_if_unset' },
				query: (args) => args,
				invalidatesTags: [providesList(ReduxTag.AuthorInfo)]
			})
		})
	});
}

export type AuthorInfo = {
	name: string | null;
	email: string | null;
};
