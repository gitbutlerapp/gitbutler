import type { Tauri } from '$lib/backend/tauri';

/**
 * Service class for listening to shortcut events from the back end.
 */
export class ShortcutService {
	private listeners: [string, CallableFunction][] = [];
	constructor(private tauri: Tauri) {}

	listen() {
		$effect(() => {
			const unsubscribe = this.tauri.listen<string>('menu://shortcut', (e) => {
				for (const listener of this.listeners) {
					if (listener[0] === e.payload) {
						listener[1]();
					}
				}
			});

			return unsubscribe;
		});
	}

	on(shortcut: string, callback: () => void) {
		$effect(() => {
			const value = [shortcut, callback] as [string, CallableFunction];
			this.listeners.push(value);
			return () => {
				this.listeners.splice(
					this.listeners.findIndex((listener) => listener === value),
					1
				);
			};
		});
	}
}
