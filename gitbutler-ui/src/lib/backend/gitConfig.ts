import { invoke } from '@tauri-apps/api/tauri';
import { writable, type Writable } from 'svelte/store';

export class GitConfig {
	gitGetConfig<T extends string>(key: string): Promise<T | null> {
		return invoke<T | null>('git_get_global_config', { key });
	}

	gitSetConfig<T extends string>(key: string, value: T) {
		return invoke<T | null>('git_set_global_config', { key, value });
	}

	buildWritable<T extends string>(key: string): Writable<T | undefined> {
		const subject = writable<T | undefined>();

		this.gitGetConfig<T>(key).then((value) => {
			subject.set(value || undefined);

			// Ensure we don't set the value we just read from the config by registering after
			subject.subscribe((recievedValue) => {
				if (recievedValue) {
					this.gitSetConfig<T>(key, recievedValue);
				}
			});
		});

		return subject;
	}

	buildWritableWithDefault<T extends string>(key: string, defaultValue: T): Writable<T> {
		const subject = writable<T>(defaultValue);

		this.gitGetConfig<T>(key).then((value) => {
			subject.set(value || defaultValue);

			// Ensure we don't set the value we just read from the config by registering after
			subject.subscribe((recievedValue) => this.gitSetConfig<T>(key, recievedValue));
		});

		return subject;
	}
}
