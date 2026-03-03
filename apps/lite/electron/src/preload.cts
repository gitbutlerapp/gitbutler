import { contextBridge, ipcRenderer } from "electron";
import type { LiteElectronApi } from "#electron/ipc";
import type {
	AssignmentRejection,
	HunkAssignmentRequest,
	TreeChange,
	CommitDetails,
	DiffSpec,
	ProjectForFrontend,
	RefInfo,
	UICommitCreateResult,
	UIMoveChangesResult,
	UnifiedPatch,
	WorktreeChanges,
} from "@gitbutler/but-sdk";

const api: LiteElectronApi = {
	async assignHunk(
		projectId: string,
		assignments: Array<HunkAssignmentRequest>,
	): Promise<Array<AssignmentRejection>> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:assign-hunk", projectId, assignments);
	},
	async changesInWorktree(projectId: string): Promise<WorktreeChanges> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:changes-in-worktree", projectId);
	},
	async commitAmend(
		projectId: string,
		commitId: string,
		changes: Array<DiffSpec>,
	): Promise<UICommitCreateResult> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:commit-amend", projectId, commitId, changes);
	},
	async commitDetailsWithLineStats(projectId: string, commitId: string): Promise<CommitDetails> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke(
			"workspace:commit-details-with-line-stats",
			projectId,
			commitId,
		);
	},
	async commitMoveChangesBetween(
		projectId: string,
		sourceCommitId: string,
		destinationCommitId: string,
		changes: Array<DiffSpec>,
	): Promise<UIMoveChangesResult> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke(
			"workspace:commit-move-changes-between",
			projectId,
			sourceCommitId,
			destinationCommitId,
			changes,
		);
	},
	async commitUncommitChanges(
		projectId: string,
		commitId: string,
		changes: Array<DiffSpec>,
		assignTo: string | null,
	): Promise<UIMoveChangesResult> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke(
			"workspace:commit-uncommit-changes",
			projectId,
			commitId,
			changes,
			assignTo,
		);
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
	async treeChangeDiffs(projectId: string, change: TreeChange): Promise<UnifiedPatch | null> {
		// oxlint-disable-next-line typescript/no-unsafe-return
		return await ipcRenderer.invoke("workspace:tree-change-diffs", projectId, change);
	},
};

contextBridge.exposeInMainWorld("lite", api);
