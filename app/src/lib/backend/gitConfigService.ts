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

	// Gets the value of `gitbutler.signCommits`
	// Determines if the app should attempt to sign commits using per the git configuration.
	async getSignCommitsConfig(projectId: string): Promise<boolean | undefined> {
		return await invoke<boolean | undefined>('get_sign_commits_config', { projectId });
	}

	// Sets the value of `gitbutler.signCommits`
	async setSignCommitsConfig(projectId: string, value: boolean) {
		return await invoke('set_sign_commits_config', { projectId, value });
	}
}
