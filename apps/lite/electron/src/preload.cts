import { contextBridge, ipcRenderer } from "electron";
import type { LiteElectronApi } from "#electron/ipc";
import type {
	ApplyOutcome,
	AssignmentRejection,
	BranchDetails,
	BranchListing,
	CommitDetails,
	ProjectForFrontend,
	RefInfo,
	TreeChanges,
	UICommitCreateResult,
	UICommitInsertBlankResult,
	UICommitMoveResult,
	UICommitRewordResult,
	UIMoveChangesResult,
	UnifiedPatch,
	WorktreeChanges,
} from "@gitbutler/but-sdk";

const api: LiteElectronApi = {
	apply: (params) => ipcRenderer.invoke("workspace:apply", params) as Promise<ApplyOutcome>,
	assignHunk: (params) =>
		ipcRenderer.invoke("workspace:assign-hunk", params) as Promise<Array<AssignmentRejection>>,
	branchDetails: (params) =>
		ipcRenderer.invoke("workspace:branch-details", params) as Promise<BranchDetails>,
	branch_diff: (params) =>
		ipcRenderer.invoke("workspace:branch-diff", params) as Promise<TreeChanges>,
	changesInWorktree: (projectId) =>
		ipcRenderer.invoke("workspace:changes-in-worktree", projectId) as Promise<WorktreeChanges>,
	commitAmend: (params) =>
		ipcRenderer.invoke("workspace:commit-amend", params) as Promise<UICommitCreateResult>,
	commitCreate: (params) =>
		ipcRenderer.invoke("workspace:commit-create", params) as Promise<UICommitCreateResult>,
	commitDetailsWithLineStats: (params) =>
		ipcRenderer.invoke(
			"workspace:commit-details-with-line-stats",
			params,
		) as Promise<CommitDetails>,
	commitInsertBlank: (params) =>
		ipcRenderer.invoke(
			"workspace:commit-insert-blank",
			params,
		) as Promise<UICommitInsertBlankResult>,
	commitMove: (params) =>
		ipcRenderer.invoke("workspace:commit-move", params) as Promise<UICommitMoveResult>,
	commitMoveToBranch: (params) =>
		ipcRenderer.invoke("workspace:commit-move-to-branch", params) as Promise<UICommitMoveResult>,
	commitReword: (params) =>
		ipcRenderer.invoke("workspace:commit-reword", params) as Promise<UICommitRewordResult>,
	commitMoveChangesBetween: (params) =>
		ipcRenderer.invoke(
			"workspace:commit-move-changes-between",
			params,
		) as Promise<UIMoveChangesResult>,
	commitUncommitChanges: (params) =>
		ipcRenderer.invoke("workspace:commit-uncommit-changes", params) as Promise<UIMoveChangesResult>,
	getVersion: () => ipcRenderer.invoke("lite:get-version") as Promise<string>,
	headInfo: (projectId) => ipcRenderer.invoke("workspace:head-info", projectId) as Promise<RefInfo>,
	listBranches: (projectId, filter) =>
		ipcRenderer.invoke("workspace:list-branches", projectId, filter) as Promise<
			Array<BranchListing>
		>,
	listProjects: () => ipcRenderer.invoke("projects:list") as Promise<Array<ProjectForFrontend>>,
	ping: (input) => ipcRenderer.invoke("lite:ping", input) as Promise<string>,
	treeChangeDiffs: (params) =>
		ipcRenderer.invoke("workspace:tree-change-diffs", params) as Promise<UnifiedPatch | null>,
	unapplyStack: (params) => ipcRenderer.invoke("workspace:unapply-stack", params) as Promise<void>,
};

contextBridge.exposeInMainWorld("lite", api);
