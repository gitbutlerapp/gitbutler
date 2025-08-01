export declare global {
	interface Window {
		/** IPC for electron. This will be present when running in electron */
		electronAPI?: {
			openDirectory(): Promise<string | undefined>;
		};
	}
}
