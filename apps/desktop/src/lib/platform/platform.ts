import { platform } from '@tauri-apps/api/os';
import { readable } from 'svelte/store';

export const platformName = readable<string | undefined>(undefined, (set) => {
	platform().then((platform) => set(platform));
});
