import type { Tauri } from '$lib/backend/tauri';

export type GitCredentialCheck = {
	error?: string;
	name?: string;
	ok: boolean;
};

export type CredentialCheckError = {
	check: string;
	message: string;
};

export class GitConfigService {
	constructor(private tauri: Tauri) {}
	async get<T extends string>(key: string): Promise<T | undefined> {
		return (await this.tauri.invoke<T | undefined>('git_get_global_config', { key })) || undefined;
	}

	async remove(key: string): Promise<undefined> {
		return await this.tauri.invoke('git_remove_global_config', { key });
	}

	async getWithDefault<T extends string>(key: string, defaultValue: T): Promise<T> {
		const value = await this.tauri.invoke<T | undefined>('git_get_global_config', { key });
		return value || defaultValue;
	}

	async set<T extends string>(key: string, value: T) {
		return await this.tauri.invoke<T | undefined>('git_set_global_config', { key, value });
	}

	async getGbConfig(projectId: string): Promise<GbConfig> {
		return await this.tauri.invoke<GbConfig>('get_gb_config', { projectId });
	}

	async setGbConfig(projectId: string, config: GbConfig) {
		return await this.tauri.invoke('set_gb_config', { projectId, config });
	}

	async checkGitFetch(projectId: string, remoteName: string | null | undefined) {
		if (!remoteName) return;
		const resp = await this.tauri.invoke<string>('git_test_fetch', {
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
		const resp = await this.tauri.invoke<string>('git_test_push', {
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
