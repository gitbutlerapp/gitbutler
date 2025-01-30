import { platform } from '@tauri-apps/plugin-os';

export const platformName = platform();

export function keysStringToArr(keys: string): string[] {
	return keys.split('+').map((key) => {
		if (key === 'Shift') return '⇧';
		if (key === '$mod') {
			if (platformName === 'macos') {
				return '⌘';
			} else {
				return 'Ctrl';
			}
		}
		return key;
	});
}
