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
	assignHunk: async (params) =>
		(await ipcRenderer.invoke("workspace:assign-hunk", params)) as Promise<
			Array<AssignmentRejection>
		>,
	changesInWorktree: async (projectId) =>
		(await ipcRenderer.invoke(
			"workspace:changes-in-worktree",
			projectId,
		)) as Promise<WorktreeChanges>,
	commitAmend: async (params) =>
		(await ipcRenderer.invoke("workspace:commit-amend", params)) as Promise<UICommitCreateResult>,
	commitDetailsWithLineStats: async (params) =>
		(await ipcRenderer.invoke(
			"workspace:commit-details-with-line-stats",
			params,
		)) as Promise<CommitDetails>,
	commitMoveChangesBetween: async (params) =>
		(await ipcRenderer.invoke(
			"workspace:commit-move-changes-between",
			params,
		)) as Promise<UIMoveChangesResult>,
	commitUncommitChanges: async (params) =>
		(await ipcRenderer.invoke(
			"workspace:commit-uncommit-changes",
			params,
		)) as Promise<UIMoveChangesResult>,
	getVersion: async () => (await ipcRenderer.invoke("lite:get-version")) as Promise<string>,
	headInfo: async (projectId) =>
		(await ipcRenderer.invoke("workspace:head-info", projectId)) as Promise<RefInfo>,
	listProjects: async () =>
		(await ipcRenderer.invoke("projects:list")) as Promise<Array<ProjectForFrontend>>,
	ping: async (input) => (await ipcRenderer.invoke("lite:ping", input)) as Promise<string>,
	treeChangeDiffs: async (params) =>
		(await ipcRenderer.invoke(
			"workspace:tree-change-diffs",
			params,
		)) as Promise<UnifiedPatch | null>,
};

contextBridge.exposeInMainWorld("lite", api);
