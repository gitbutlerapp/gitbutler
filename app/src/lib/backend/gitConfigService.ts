import { invoke } from '$lib/backend/ipc';

export class GitConfigService {
	async get<T extends string>(key: string): Promise<T | undefined> {
		return (await invoke<T | undefined>('git_get_global_config', { key })) || undefined;
	}

	async getWithDefault<T extends string>(key: string, defaultValue: T): Promise<T> {
		const value = await invoke<T | undefined>('git_get_global_config', { key });
		return value || defaultValue;
	}

	async set<T extends string>(key: string, value: T) {
		return await invoke<T | undefined>('git_set_global_config', { key, value });
	}

	async getGbConfig(projectId: string): Promise<GbConfig> {
		return await invoke<GbConfig>('get_gb_config', { projectId });
	}

	async setGbConfig(projectId: string, config: GbConfig) {
		return await invoke('set_gb_config', { projectId, config });
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
