import { determinePlatform } from '$lib/utils/platform';

export function keysStringToArr(keys: string): string[] {
	const platform = determinePlatform(navigator.userAgent);

	return keys.split('+').map((key) => {
		if (key === 'Shift') return '⇧';
		if (key === '$mod') {
			if (platform === 'macos') {
				return '⌘';
			} else {
				return 'Ctrl';
			}
		}
		return key;
	});
}
