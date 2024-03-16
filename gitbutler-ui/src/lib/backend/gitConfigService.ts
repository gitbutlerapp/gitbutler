import { invoke } from '@tauri-apps/api/tauri';

export class GitConfigService {
	async get<T extends string>(key: string): Promise<T | undefined> {
		return await invoke<T | undefined>('git_get_global_config', { key });
	}

	async getWithDefault<T extends string>(key: string, defaultValue: T): Promise<T> {
		const value = await invoke<T | undefined>('git_get_global_config', { key });
		return value || defaultValue;
	}

	async set<T extends string>(key: string, value: T) {
		return invoke<T | undefined>('git_set_global_config', { key, value });
	}
}
