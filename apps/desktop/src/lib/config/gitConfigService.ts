import { invalidatesItem, invalidatesList, providesItem, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/core/context';
import type { IBackend } from '$lib/backend';
import type { ClientState } from '$lib/state/clientState.svelte';

export const GIT_CONFIG_SERVICE = new InjectionToken<GitConfigService>('GitConfigService');

export class GitConfigService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		private clientApi: ClientState,
		private backend: IBackend
	) {
		this.api = injectEndpoints(clientApi.backendApi);
	}
	async get<T extends string>(key: string): Promise<T | undefined> {
		return ((await this.api.endpoints.gitGetGlobalConfig.fetch({ key })) as T) ?? undefined;
	}

	async remove(key: string): Promise<undefined> {
		return await this.api.endpoints.gitRemoveGlobalConfig.mutate({ key });
	}

	async getWithDefault<T extends string>(key: string, defaultValue: T): Promise<T> {
		const value = await this.get<T>(key);
		return value || defaultValue;
	}

	async set<T extends string>(key: string, value: T) {
		return await this.api.endpoints.gitSetGlobalConfig.mutate({ key, value });
	}

	invalidateGitConfig() {
		this.clientApi.dispatch(
			this.api.util.invalidateTags([invalidatesList(ReduxTag.GitConfigProperty)])
		);
	}

	async getGbConfig(projectId: string): Promise<GbConfig> {
		return await this.backend.invoke<GbConfig>('get_gb_config', { projectId });
	}

	async setGbConfig(projectId: string, config: GbConfig) {
		return await this.backend.invoke('set_gb_config', { projectId, config });
	}

	async checkGitFetch(projectId: string, remoteName: string | null | undefined) {
		if (!remoteName) return;
		const resp = await this.backend.invoke<string>('git_test_fetch', {
			projectId: projectId,
			action: 'modal',
			remoteName
		});
		// fix: we should have a response with an optional error
		if (resp) throw new Error(resp);
		return;
	}

	async checkGitPush(
		projectId: string,
		remoteName: string | null | undefined,
		branchName: string | null | undefined
	) {
		if (!remoteName) return;
		const resp = await this.backend.invoke<string>('git_test_push', {
			projectId: projectId,
			action: 'modal',
			remoteName,
			branchName
		});
		if (resp) throw new Error(resp);
		return { name: 'push', ok: true };
	}
}

// These are git configuration values that are read and set by gitbutler.
// Let's use this config type as a middle ground between setting string key/values from the frontend and having separate get/set methods for each key.
// This way we can keep track on both frontend and rust-end what is being used and for what purpose.
export class GbConfig {
	signCommits?: boolean | undefined;
	signingKey?: string | undefined;
	signingFormat?: string | undefined;
	gpgProgram?: string | undefined;
	gpgSshProgram?: string | undefined;
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			gitGetGlobalConfig: build.query<unknown, { key: string }>({
				keepUnusedDataFor: 30,
				extraOptions: { command: 'git_get_global_config' },
				query: (args) => args,
				transformResponse: (response: unknown) => {
					return response;
				},
				providesTags: (_result, _error, args) => providesItem(ReduxTag.GitConfigProperty, args.key)
			}),
			gitRemoveGlobalConfig: build.mutation<undefined, { key: string }>({
				extraOptions: { command: 'git_remove_global_config' },
				query: (args) => args,
				invalidatesTags: (_result, _error, args) =>
					invalidatesItem(ReduxTag.GitConfigProperty, args.key)
			}),
			gitSetGlobalConfig: build.mutation<unknown, { key: string; value: unknown }>({
				extraOptions: { command: 'git_set_global_config' },
				query: (args) => args,
				invalidatesTags: (_result, _error, args) =>
					invalidatesItem(ReduxTag.GitConfigProperty, args.key)
			})
		})
	});
}
