import { InjectionToken } from "@gitbutler/core/context";
import type { IBackend } from "$lib/backend";
import type { BackendApi } from "$lib/state/backendApi";

export const GIT_SERVICE = new InjectionToken<GitService>("GitService");

export class GitService {
	constructor(
		private backend: IBackend,
		private backendApi: BackendApi,
	) {}

	/**
	 * Emits a new value when a fetch was detected by the back end.
	 * @example
	 * $effect(() => gitService.onFetch(data.projectId, () => {}));
	 */
	onFetch(projectId: string, callback: () => void) {
		return this.backend.listen<any>(`project://${projectId}/git/fetch`, callback);
	}

	async checkSigningSettings(projectId: string): Promise<void> {
		return await this.backend.invoke("check_signing_settings", { projectId });
	}

	async indexSize(projectId: string): Promise<number> {
		return await this.backend.invoke("git_index_size", { projectId });
	}

	async cloneRepo(repoUrl: string, dir: string): Promise<void> {
		await this.backend.invoke("git_clone_repository", {
			repositoryUrl: repoUrl,
			targetDir: dir,
		});
	}

	getAuthorInfo(projectId: string) {
		return this.backendApi.endpoints.authorInfo.useQuery({ projectId });
	}

	get setAuthorInfo() {
		return this.backendApi.endpoints.setAuthorInfo.useMutation();
	}
}
