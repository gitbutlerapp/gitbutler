import { Transform, Type } from 'class-transformer';

export type Operation =
	| 'CreateCommit'
	| 'CreateBranch'
	| 'SetBaseBranch'
	| 'MergeUpstream'
	| 'UpdateWorkspaceBase'
	| 'MoveHunk'
	| 'UpdateBranchName'
	| 'UpdateBranchNotes'
	| 'ReorderBranches'
	| 'SelectDefaultVirtualBranch'
	| 'UpdateBranchRemoteName'
	| 'GenericBranchUpdate'
	| 'DeleteBranch'
	| 'ApplyBranch'
	| 'DiscardLines'
	| 'DiscardHunk'
	| 'DiscardFile'
	| 'AmendCommit'
	| 'UndoCommit'
	| 'UnapplyBranch'
	| 'CherryPick'
	| 'SquashCommit'
	| 'UpdateCommitMessage'
	| 'MoveCommit'
	| 'MoveBranch'
	| 'TearOffBranch'
	| 'RestoreFromSnapshot'
	| 'ReorderCommit'
	| 'InsertBlankCommit'
	| 'MoveCommitFile'
	| 'FileChanges'
	| 'EnterEditMode'
	| 'SyncWorkspace'
	| 'CreateDependentBranch'
	| 'RemoveDependentBranch'
	| 'UpdateDependentBranchName'
	| 'UpdateDependentBranchDescription'
	| 'UpdateDependentBranchPrNumber'
	| 'AutoHandleChangesBefore'
	| 'AutoHandleChangesAfter'
	| 'SplitBranch';

export class Trailer {
	key!: string;
	value!: string;
}

export class SnapshotDetails {
	title!: string;
	operation!: Operation;
	body?: string | undefined;
	@Type(() => Trailer)
	trailers!: Trailer[];
}

export class Snapshot {
	id!: string;
	@Type(() => SnapshotDetails)
	details?: SnapshotDetails;
	@Transform((obj) => new Date(obj.value * 1000))
	createdAt!: Date;
}
