import type { LiteElectronApi } from "#electron/ipc.ts";

declare global {
	interface Window {
		lite: LiteElectronApi;
	}
}

export {};
