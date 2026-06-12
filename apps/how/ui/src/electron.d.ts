import type { HowElectronApi } from "../../electron/src/ipc";

declare global {
	interface Window {
		how: HowElectronApi;
	}
}

export {};
