import { invalidatesItem, invalidatesList, providesItem, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { IBackend } from "$lib/backend";
import type { ClientState } from "$lib/state/clientState.svelte";
import type { GitConfigSettings } from "@gitbutler/core/api";

export const GIT_CONFIG_SERVICE = new InjectionToken<GitConfigService>("GitConfigService");

export class GitConfigService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		private clientApi: ClientState,
		private backend: IBackend,
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
			this.api.util.invalidateTags([invalidatesList(ReduxTag.GitConfigProperty)]),
		);
	}

	gbConfig(projectId: string) {
		return this.api.endpoints.gbConfig.useQuery({ projectId });
	}

	async getGbConfig(projectId: string): Promise<GitConfigSettings> {
		return await this.api.endpoints.gbConfig.fetch({ projectId });
	}

	async setGbConfig(projectId: string, config: Partial<GitConfigSettings>) {
		return await this.api.endpoints.setGbConfig.mutate({
			projectId,
			config: {
				signCommits: config.signCommits ?? null,
				gitbutlerGerritMode: config.gitbutlerGerritMode ?? null,
				gitbutlerForgeReviewTemplatePath: config.gitbutlerForgeReviewTemplatePath ?? null,
				gitbutlerGitlabProjectId: config.gitbutlerGitlabProjectId ?? null,
				gitbutlerGitlabUpstreamProjectId: config.gitbutlerGitlabUpstreamProjectId ?? null,
				signingKey: config.signingKey ?? null,
				signingFormat: config.signingFormat ?? null,
				gpgProgram: config.gpgProgram ?? null,
				gpgSshProgram: config.gpgSshProgram ?? null,
			},
		});
	}

	async setGerritMode(projectId: string, gerritMode: boolean) {
		return await this.setGbConfig(projectId, { gitbutlerGerritMode: gerritMode });
	}

	async setForgeReviewTemplatePath(projectId: string, path: string | null) {
		return await this.setGbConfig(projectId, { gitbutlerForgeReviewTemplatePath: path });
	}

	async checkGitFetch(projectId: string, remoteName: string | null | undefined) {
		if (!remoteName) return;
		const resp = await this.backend.invoke<string>("git_test_fetch", {
			projectId: projectId,
			action: "modal",
			remoteName,
		});
		// fix: we should have a response with an optional error
		if (resp) throw new Error(resp);
		return;
	}

	async checkGitPush(
		projectId: string,
		remoteName: string | null | undefined,
		branchName: string | null | undefined,
	) {
		if (!remoteName) return;
		const resp = await this.backend.invoke<string>("git_test_push", {
			projectId: projectId,
			action: "modal",
			remoteName,
			branchName,
		});
		if (resp) throw new Error(resp);
		return { name: "push", ok: true };
	}
}

function injectEndpoints(api: ClientState["backendApi"]) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			gitGetGlobalConfig: build.query<unknown, { key: string }>({
				keepUnusedDataFor: 30,
				extraOptions: { command: "git_get_global_config" },
				query: (args) => args,
				transformResponse: (response: unknown) => {
					return response;
				},
				providesTags: (_result, _error, args) => providesItem(ReduxTag.GitConfigProperty, args.key),
			}),
			gitRemoveGlobalConfig: build.mutation<undefined, { key: string }>({
				extraOptions: { command: "git_remove_global_config" },
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.GitConfigProperty, args.key),
				],
			}),
			gitSetGlobalConfig: build.mutation<unknown, { key: string; value: unknown }>({
				extraOptions: { command: "git_set_global_config" },
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.GitConfigProperty, args.key),
				],
			}),
			gbConfig: build.query<GitConfigSettings, { projectId: string }>({
				extraOptions: { command: "get_gb_config" },
				query: (args) => args,
				providesTags: (_result, _error, args) =>
					providesItem(ReduxTag.GitButlerConfig, args.projectId),
			}),
			setGbConfig: build.mutation<void, { projectId: string; config: GitConfigSettings }>({
				extraOptions: { command: "set_gb_config" },
				query: (args) => args,
				invalidatesTags: (_result, _error, args) => [
					invalidatesItem(ReduxTag.GitButlerConfig, args.projectId),
					invalidatesItem(ReduxTag.Project, args.projectId),
				],
			}),
		}),
	});
}
