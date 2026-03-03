import { contextBridge, ipcRenderer } from "electron";
import type {
	AssignHunkParams,
	CommitAmendParams,
	CommitDetailsWithLineStatsParams,
	CommitMoveChangesBetweenParams,
	CommitUncommitChangesParams,
	LiteElectronApi,
	TreeChangeDiffParams,
} from "#electron/ipc";
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
	async assignHunk(params: AssignHunkParams): Promise<Array<AssignmentRejection>> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:assign-hunk", params);
	},
	async changesInWorktree(projectId: string): Promise<WorktreeChanges> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:changes-in-worktree", projectId);
	},
	async commitAmend(params: CommitAmendParams): Promise<UICommitCreateResult> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:commit-amend", params);
	},
	async commitDetailsWithLineStats(
		params: CommitDetailsWithLineStatsParams,
	): Promise<CommitDetails> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:commit-details-with-line-stats", params);
	},
	async commitMoveChangesBetween(
		params: CommitMoveChangesBetweenParams,
	): Promise<UIMoveChangesResult> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:commit-move-changes-between", params);
	},
	async commitUncommitChanges(params: CommitUncommitChangesParams): Promise<UIMoveChangesResult> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:commit-uncommit-changes", params);
	},
	async getVersion(): Promise<string> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("lite:get-version");
	},
	async headInfo(projectId: string): Promise<RefInfo> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:head-info", projectId);
	},
	async listProjects(): Promise<Array<ProjectForFrontend>> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("projects:list");
	},
	async ping(input: string): Promise<string> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("lite:ping", input);
	},
	async treeChangeDiffs(params: TreeChangeDiffParams): Promise<UnifiedPatch | null> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:tree-change-diffs", params);
	},
};

contextBridge.exposeInMainWorld("lite", api);
