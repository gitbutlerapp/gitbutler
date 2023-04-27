import { building } from '$app/environment';
import type Events from '$lib/events';

export default async (events: ReturnType<typeof Events>) =>
	building
		? { on: () => () => {} }
		: await import('tinykeys').then(({ default: tinykeys }) => ({
				on: (combo: string, callback: (event: KeyboardEvent) => void) => {
					if (building) return () => {};
					const comboContainsControlKeys =
						combo.includes('Meta') || combo.includes('Alt') || combo.includes('Ctrl');
					return tinykeys(window, {
						[combo]: (event) => {
							const target = event.target as HTMLElement;
							const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA';
							if (isInput && !comboContainsControlKeys) return;

							event.preventDefault();
							event.stopPropagation();

							events.closeCommandPalette();
							callback(event);
						}
					});
				}
		  }));
