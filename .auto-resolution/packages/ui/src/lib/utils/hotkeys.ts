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

export enum KeyName {
	Space = ' ',
	Meta = 'Meta',
	Alt = 'Alt',
	Ctrl = 'Ctrl',
	Enter = 'Enter',
	Escape = 'Escape',
	Tab = 'Tab',
	Up = 'ArrowUp',
	Down = 'ArrowDown',
	Left = 'ArrowLeft',
	Right = 'ArrowRight',
	Delete = 'Backspace'
}

export function onMetaEnter(callback: () => void) {
	return (e: KeyboardEvent) => {
		if (e.key === KeyName.Enter && (e.metaKey || e.ctrlKey)) {
			callback();
		}
	};
}
