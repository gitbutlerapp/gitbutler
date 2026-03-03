import type { ProjectForFrontend, RefInfo } from "@gitbutler/but-sdk";

export interface LiteElectronApi {
	ping(input: string): Promise<string>;
	getVersion(): Promise<string>;
	listProjects(): Promise<Array<ProjectForFrontend>>;
	headInfo(projectId: string): Promise<RefInfo>;
}

export const liteIpcChannels = {
	ping: "lite:ping",
	getVersion: "lite:get-version",
	listProjects: "projects:list",
	headInfo: "workspace:head-info",
} as const;
