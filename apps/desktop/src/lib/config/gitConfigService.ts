import { InjectionToken } from '@gitbutler/core/context';
import type { IBackend } from '$lib/backend';

export const GIT_CONFIG_SERVICE = new InjectionToken<GitConfigService>('GitConfigService');

export interface IGitConfigService {
	get<T extends string>(key: string): Promise<T | undefined>;
	remove(key: string): Promise<undefined>;
	getWithDefault<T extends string>(key: string, defaultValue: T): Promise<T>;
	set<T extends string>(key: string, value: T): Promise<T | undefined>;
	getGbConfig(projectId: string): Promise<GbConfig>;
	setGbConfig(projectId: string, config: GbConfig): Promise<void>;
	checkGitFetch(projectId: string, remoteName: string | null | undefined): Promise<void>;
	checkGitPush(
		projectId: string,
		remoteName: string | null | undefined,
		branchName: string | null | undefined
	): Promise<{ name: string; ok: boolean }>;
}

export class GitConfigService implements IGitConfigService {
	constructor(private backend: IBackend) {}
	async get<T extends string>(key: string): Promise<T | undefined> {
		return (
			(await this.backend.invoke<T | undefined>('git_get_global_config', { key })) || undefined
		);
	}

	async remove(key: string): Promise<undefined> {
		return await this.backend.invoke('git_remove_global_config', { key });
	}

	async getWithDefault<T extends string>(key: string, defaultValue: T): Promise<T> {
		const value = await this.backend.invoke<T | undefined>('git_get_global_config', { key });
		return value || defaultValue;
	}

	async set<T extends string>(key: string, value: T) {
		return await this.backend.invoke<T | undefined>('git_set_global_config', { key, value });
	}

	async getGbConfig(projectId: string): Promise<GbConfig> {
		return await this.backend.invoke<GbConfig>('get_gb_config', { projectId });
	}

	async setGbConfig(projectId: string, config: GbConfig): Promise<void> {
		return await this.backend.invoke('set_gb_config', { projectId, config });
	}

	async checkGitFetch(projectId: string, remoteName: string | null | undefined): Promise<void> {
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
	): Promise<{ name: string; ok: boolean }> {
		if (!remoteName) return { name: 'push', ok: true };
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
