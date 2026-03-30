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
	| "UnapplyBranch"
	| "CherryPick"
	| "SquashCommit"
	| "UpdateCommitMessage"
	| "MoveCommit"
	| "MoveBranch"
	| "TearOffBranch"
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
	id: string;
	details?: SnapshotDetails;
	/** Milliseconds since epoch. */
	createdAt: number;
}
