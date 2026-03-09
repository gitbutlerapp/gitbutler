import { contextBridge, ipcRenderer } from "electron";
import type { LiteElectronApi } from "#electron/ipc";
import type {
	AssignmentRejection,
	CommitDetails,
	ProjectForFrontend,
	RefInfo,
	UICommitCreateResult,
	UIMoveChangesResult,
	UnifiedPatch,
	WorktreeChanges,
} from "@gitbutler/but-sdk";

const api: LiteElectronApi = {
	async assignHunk(params) {
		return (await ipcRenderer.invoke("workspace:assign-hunk", params)) as Promise<
			Array<AssignmentRejection>
		>;
	},
	async changesInWorktree(projectId) {
		return (await ipcRenderer.invoke(
			"workspace:changes-in-worktree",
			projectId,
		)) as Promise<WorktreeChanges>;
	},
	async commitAmend(params) {
		return (await ipcRenderer.invoke(
			"workspace:commit-amend",
			params,
		)) as Promise<UICommitCreateResult>;
	},
	async commitDetailsWithLineStats(params) {
		return (await ipcRenderer.invoke(
			"workspace:commit-details-with-line-stats",
			params,
		)) as Promise<CommitDetails>;
	},
	async commitMoveChangesBetween(params) {
		return (await ipcRenderer.invoke(
			"workspace:commit-move-changes-between",
			params,
		)) as Promise<UIMoveChangesResult>;
	},
	async commitUncommitChanges(params) {
		return (await ipcRenderer.invoke(
			"workspace:commit-uncommit-changes",
			params,
		)) as Promise<UIMoveChangesResult>;
	},
	async getVersion() {
		return (await ipcRenderer.invoke("lite:get-version")) as Promise<string>;
	},
	async headInfo(projectId) {
		return (await ipcRenderer.invoke("workspace:head-info", projectId)) as Promise<RefInfo>;
	},
	async listProjects() {
		return (await ipcRenderer.invoke("projects:list")) as Promise<Array<ProjectForFrontend>>;
	},
	async ping(input) {
		return (await ipcRenderer.invoke("lite:ping", input)) as Promise<string>;
	},
	async treeChangeDiffs(params) {
		return (await ipcRenderer.invoke(
			"workspace:tree-change-diffs",
			params,
		)) as Promise<UnifiedPatch | null>;
	},
};

contextBridge.exposeInMainWorld("lite", api);
