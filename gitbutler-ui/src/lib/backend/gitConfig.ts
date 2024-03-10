import { invoke } from '@tauri-apps/api/tauri';

export class GitConfig {
	gitGetConfig<T extends string>(key: string): Promise<T | null> {
		return invoke<T | null>('git_get_global_config', { key });
	}

	gitSetConfig<T extends string>(key: string, value: T) {
		return invoke<T | null>('git_set_global_config', { key, value });
	}

	buildGetter<T extends string>(key: string) {
		return () => this.gitGetConfig<T>(key);
	}

	buildGetterWithDefault<T extends string>(key: string, defaultValue: T): () => Promise<T> {
		return () => this.gitGetConfig(key).then((value) => value || defaultValue) as Promise<T>;
	}

	buildSetter<T extends string>(key: string) {
		return (value: T) => this.gitSetConfig<T>(key, value);
	}
}
