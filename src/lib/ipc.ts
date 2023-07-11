import type { EventCallback, EventName } from '@tauri-apps/api/event';
import { invoke as invokeTauri } from '@tauri-apps/api/tauri';
import { listen as listenTauri } from '@tauri-apps/api/event';

export async function invoke<T>(command: string, params: Record<string, unknown> = {}): Promise<T> {
	return invokeTauri<T>(command, params);
}

export function listen<T>(event: EventName, handle: EventCallback<T>) {
	return listenTauri(event, handle);
}
