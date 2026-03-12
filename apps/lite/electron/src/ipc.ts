import type { ProjectForFrontend, RefInfo } from "@gitbutler/but-sdk";

export type LongRunningTaskStatus = "running" | "cancelling" | "done" | "cancelled" | "error";

export interface LongRunningTaskSnapshot {
	taskId: number;
	durationMs: number;
	step: number;
	status: LongRunningTaskStatus;
	message?: string;
}

export interface LiteElectronApi {
	listProjects(): Promise<ProjectForFrontend[]>;
	headInfo(projectId: string): Promise<RefInfo>;
	listLongRunningTasks(): Promise<LongRunningTaskSnapshot[]>;
	startLongRunningTask(durationMs: number): Promise<number>;
	cancelLongRunningTask(taskId: number): Promise<boolean>;
	onLongRunningTaskEvent(listener: (event: LongRunningTaskSnapshot) => void): () => void;
}

export const liteIpcChannels = {
	ping: "lite:ping",
	getVersion: "lite:get-version",
	listProjects: "projects:list",
	headInfo: "workspace:head-info",
	listLongRunningTasks: "long-running:list",
	startLongRunningTask: "long-running:start",
	cancelLongRunningTask: "long-running:cancel",
	longRunningTaskEvent: "long-running:event",
} as const;
