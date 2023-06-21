import { building } from '$app/environment';
import type { EventCallback, EventName } from '@tauri-apps/api/event';

export async function invoke<T>(command: string, params: Record<string, unknown> = {}): Promise<T> {
	return building
		? Promise.reject('cannot invoke ipc command while building')
		: import('@tauri-apps/api').then(({ invoke }) => invoke<T>(command, params));
}

export function listen<T>(event: EventName, handle: EventCallback<T>) {
	return building
		? Promise.reject('cannot listen to ipc events while building')
		: import('@tauri-apps/api/event').then(({ listen }) => listen(event, handle));
}
