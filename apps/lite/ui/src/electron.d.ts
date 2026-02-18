import type { LiteElectronApi } from '#electron/ipc';

declare global {
	interface Window {
		lite: LiteElectronApi;
	}
}

export {};
