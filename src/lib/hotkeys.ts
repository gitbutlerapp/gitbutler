import { building } from '$app/environment';
import tinykeys from 'tinykeys';
import type Events from '$lib/events';

export default (events: ReturnType<typeof Events>) => ({
	on: (combo: string, callback: (event: KeyboardEvent) => void) => {
		if (building) return () => {};
		const comboContainsControlKeys =
			combo.includes('Meta') || combo.includes('Alt') || combo.includes('Ctrl');
		return tinykeys(window, {
			[combo]: (event) => {
				event.preventDefault();
				event.stopPropagation();

				const target = event.target as HTMLElement;
				const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA';
				if (isInput && !comboContainsControlKeys) return;
				events.closeCommandPalette();
				callback(event);
			}
		});
	}
});
