import type { Tauri } from '$lib/backend/tauri';

/**
 * Service class for listening to shortcut events from the back end.
 */
export class ShortcutService {
	private listeners: [string, CallableFunction][] = [];
	constructor(private tauri: Tauri) {}

	listen() {
		$effect(() =>
			this.tauri.listen<string>('menu://shortcut', (e) => {
				for (const listener of this.listeners) {
					if (listener[0] === e.payload) {
						listener[1]();
					}
				}
			})
		);
	}

	on(shortcut: string, callback: () => void) {
		$effect(() => {
			this.listeners.push([shortcut, callback]);
			return () => {
				this.listeners.splice(this.listeners.findIndex(callback), 1);
			};
		});
	}
}
