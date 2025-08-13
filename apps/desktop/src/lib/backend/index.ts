import Tauri from '$lib/backend/tauri';
import Web from '$lib/backend/web';
import { InjectionToken } from '@gitbutler/shared/context';
import type { IBackend } from '$lib/backend/backend';

export const BACKEND = new InjectionToken<IBackend>('Backend');

export default function createBackend(): IBackend {
	if (import.meta.env.VITE_BUILD_TARGET === 'web') {
		return new Web();
	}
	return new Tauri();
}

export function isBackend(something: unknown): something is IBackend {
	return (
		typeof something === 'object' &&
		something !== null &&
		(something instanceof Tauri || something instanceof Web)
	);
}

export * from '$lib/backend/backend';
