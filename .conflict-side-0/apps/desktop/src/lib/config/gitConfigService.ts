import { invalidatesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { IBackend } from "$lib/backend";
import type { BackendApi } from "$lib/state/backendApi";
import type { AppDispatch } from "$lib/state/clientState.svelte";
import type { GitConfigSettings } from "@gitbutler/but-sdk";

export const GIT_CONFIG_SERVICE = new InjectionToken<GitConfigService>("GitConfigService");

export class GitConfigService {
	constructor(
		private backendApi: BackendApi,
		private dispatch: AppDispatch,
		private backend: IBackend,
	) {}

	async get<T extends string>(key: string): Promise<T | undefined> {
		return ((await this.backendApi.endpoints.gitGetGlobalConfig.fetch({ key })) as T) ?? undefined;
	}

	async remove(key: string): Promise<undefined> {
		return await this.backendApi.endpoints.gitRemoveGlobalConfig.mutate({ key });
	}

	async getWithDefault<T extends string>(key: string, defaultValue: T): Promise<T> {
		const value = await this.get<T>(key);
		return value || defaultValue;
	}

	async set<T extends string>(key: string, value: T) {
		return await this.backendApi.endpoints.gitSetGlobalConfig.mutate({ key, value });
	}

	invalidateGitConfig() {
		this.dispatch(
			this.backendApi.util.invalidateTags([invalidatesList(ReduxTag.GitConfigProperty)]),
		);
	}

	gbConfig(projectId: string) {
		return this.backendApi.endpoints.gbConfig.useQuery({ projectId });
	}

	async getGbConfig(projectId: string): Promise<GitConfigSettings> {
		return await this.backendApi.endpoints.gbConfig.fetch({ projectId });
	}

	async setGbConfig(projectId: string, config: Partial<GitConfigSettings>) {
		return await this.backendApi.endpoints.setGbConfig.mutate({
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
