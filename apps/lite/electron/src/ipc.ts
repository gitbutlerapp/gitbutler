export interface LiteElectronApi {
	ping(input: string): Promise<string>;
	getVersion(): Promise<string>;
}

export const liteIpcChannels = {
	ping: 'lite:ping',
	getVersion: 'lite:get-version'
} as const;
