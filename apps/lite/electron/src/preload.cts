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
	assignHunk: (params) =>
		ipcRenderer.invoke("workspace:assign-hunk", params) as Promise<Array<AssignmentRejection>>,
	changesInWorktree: (projectId) =>
		ipcRenderer.invoke("workspace:changes-in-worktree", projectId) as Promise<WorktreeChanges>,
	commitAmend: (params) =>
		ipcRenderer.invoke("workspace:commit-amend", params) as Promise<UICommitCreateResult>,
	commitDetailsWithLineStats: (params) =>
		ipcRenderer.invoke(
			"workspace:commit-details-with-line-stats",
			params,
		) as Promise<CommitDetails>,
	commitMoveChangesBetween: (params) =>
		ipcRenderer.invoke(
			"workspace:commit-move-changes-between",
			params,
		) as Promise<UIMoveChangesResult>,
	commitUncommitChanges: (params) =>
		ipcRenderer.invoke("workspace:commit-uncommit-changes", params) as Promise<UIMoveChangesResult>,
	getVersion: () => ipcRenderer.invoke("lite:get-version") as Promise<string>,
	headInfo: (projectId) => ipcRenderer.invoke("workspace:head-info", projectId) as Promise<RefInfo>,
	listProjects: () => ipcRenderer.invoke("projects:list") as Promise<Array<ProjectForFrontend>>,
	ping: (input) => ipcRenderer.invoke("lite:ping", input) as Promise<string>,
	treeChangeDiffs: (params) =>
		ipcRenderer.invoke("workspace:tree-change-diffs", params) as Promise<UnifiedPatch | null>,
};

contextBridge.exposeInMainWorld("lite", api);
