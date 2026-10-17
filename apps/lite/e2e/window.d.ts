import type { LiteElectronApi } from "../electron/src/ipc.ts";

declare global {
	interface Window {
		lite: LiteElectronApi;
	}
}

export {};
