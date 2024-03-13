import { invoke } from '@tauri-apps/api/tauri';

export const GIT_CONFING_CONTEXT = Symbol();

export class GitConfig {
	get<T extends string>(key: string): Promise<T | null> {
		return invoke<T | null>('git_get_global_config', { key });
	}

	set<T extends string>(key: string, value: T) {
		return invoke<T | null>('git_set_global_config', { key, value });
	}
}
