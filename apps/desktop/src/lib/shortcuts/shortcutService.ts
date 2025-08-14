import { InjectionToken } from '@gitbutler/shared/context';
import type { IBackend } from '$lib/backend';

export const SHORTCUT_SERVICE = new InjectionToken<ShortcutService>('ShortcutService');

/**
 * Service class for listening to shortcut events from the back end.
 */
export class ShortcutService {
	private listeners: [string, CallableFunction][] = [];
	constructor(private backend: IBackend) {}

	listen() {
		return this.backend.listen<string>('menu://shortcut', (e) => {
			for (const listener of this.listeners) {
				if (listener[0] === e.payload) {
					listener[1]();
				}
			}
		});
	}

	on(shortcut: string, callback: () => void) {
		const value = [shortcut, callback] as [string, CallableFunction];
		this.listeners.push(value);
		return () => {
			this.listeners.splice(
				this.listeners.findIndex((listener) => listener === value),
				1
			);
		};
	}
}
