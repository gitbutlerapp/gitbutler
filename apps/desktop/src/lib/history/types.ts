export type Operation =
	| "CreateCommit"
	| "CreateBranch"
	| "SetBaseBranch"
	| "MergeUpstream"
	| "UpdateWorkspaceBase"
	| "MoveHunk"
	| "UpdateBranchName"
	| "UpdateBranchNotes"
	| "ReorderBranches"
	| "SelectDefaultVirtualBranch"
	| "UpdateBranchRemoteName"
	| "GenericBranchUpdate"
	| "DeleteBranch"
	| "ApplyBranch"
	| "DiscardLines"
	| "DiscardHunk"
	| "DiscardFile"
	| "AmendCommit"
	| "Absorb"
	| "AutoCommit"
	| "UndoCommit"
	| "DiscardCommit"
	| "UnapplyBranch"
	| "CherryPick"
	| "SquashCommit"
	| "UpdateCommitMessage"
	| "MoveCommit"
	| "MoveBranch"
	| "TearOffBranch"
	| "RestoreFromSnapshotViaUndo"
	| "RestoreFromSnapshotViaRedo"
	| "RestoreFromSnapshot"
	| "ReorderCommit"
	| "InsertBlankCommit"
	| "MoveCommitFile"
	| "FileChanges"
	| "EnterEditMode"
	| "SyncWorkspace"
	| "CreateDependentBranch"
	| "RemoveDependentBranch"
	| "UpdateDependentBranchName"
	| "UpdateDependentBranchDescription"
	| "UpdateDependentBranchPrNumber"
	| "AutoHandleChangesBefore"
	| "AutoHandleChangesAfter"
	| "SplitBranch"
	| "OnDemandSnapshot";

export interface Trailer {
	key: string;
	value: string;
}

export interface SnapshotDetails {
	title: string;
	operation: Operation;
	body?: string | undefined;
	trailers: Trailer[];
}

export interface Snapshot {
	/** Wire field is `commitId`; see `but_api::legacy::oplog::json::Snapshot`. */
	commitId: string;
	details?: SnapshotDetails;
	/** Milliseconds since epoch. */
	createdAt: number;
}
