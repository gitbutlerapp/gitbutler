import { building } from '$app/environment';
import { events } from '$lib';

export const on = async (combo: string, callback: (event: KeyboardEvent) => void) => {
	if (building) return () => {};

	const comboContainsControlKeys =
		combo.includes('Meta') || combo.includes('Alt') || combo.includes('Ctrl');

	return import('tinykeys').then(({ default: tinykeys }) =>
		tinykeys(window, {
			[combo]: (event) => {
				const target = event.target as HTMLElement;
				const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA';
				if (isInput && !comboContainsControlKeys) return;

				event.preventDefault();
				event.stopPropagation();

				events.emit('closeCommandPalette');
				callback(event);
			}
		})
	);
};
