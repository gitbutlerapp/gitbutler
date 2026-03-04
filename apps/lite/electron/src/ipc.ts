import type { ProjectForFrontend, RefInfo } from "@gitbutler/but-sdk";

export interface LiteElectronApi {
	getVersion(): Promise<string>;
	headInfo(projectId: string): Promise<RefInfo>;
	listProjects(): Promise<Array<ProjectForFrontend>>;
	ping(input: string): Promise<string>;
}

export const liteIpcChannels = {
	getVersion: "lite:get-version",
	headInfo: "workspace:head-info",
	listProjects: "projects:list",
	ping: "lite:ping",
} as const;
