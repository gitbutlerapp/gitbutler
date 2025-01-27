import type { Tauri } from '$lib/backend/tauri';

type ShortcutListener = {
	id: number;
	shortcut: string;
	callback: () => void;
};

/**
 * Service class for listening to shortcut events from the back end.
 */
export class ShortcutService {
	static idCounter = 0;
	private listeners: ShortcutListener[] = [];
	constructor(private tauri: Tauri) {}

	listen() {
		$effect(() =>
			this.tauri.listen<string>('menu://shortcut', (e) => {
				for (const listener of this.listeners) {
					if (listener.shortcut === e.payload) {
						listener.callback();
					}
				}
			})
		);
	}

	on(shortcut: string, callback: () => void) {
		const id = ShortcutService.idCounter;
		ShortcutService.idCounter += 1;
		const listener = { id, shortcut, callback };
		$effect(() => {
			this.listeners.push(listener);
			return () => {
				const index = this.listeners.findIndex((l) => l.id === id);
				if (index >= 0) this.listeners.splice(index, 1);
			};
		});
	}
}
