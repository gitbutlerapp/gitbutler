import { ProjectForFrontend } from '@gitbutler/but-sdk';

export interface LiteElectronApi {
	ping(input: string): Promise<string>;
	getVersion(): Promise<string>;
	listProjects(): Promise<ProjectForFrontend[]>;
}

export const liteIpcChannels = {
	ping: 'lite:ping',
	getVersion: 'lite:get-version',
	listProjects: 'projects:list'
} as const;
