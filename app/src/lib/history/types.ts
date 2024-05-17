import { RemoteHunk } from '$lib/vbranches/types';
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
	| 'DiscardHunk'
	| 'DiscardFile'
	| 'AmendCommit'
	| 'UndoCommit'
	| 'UnapplyBranch'
	| 'CherryPick'
	| 'SquashCommit'
	| 'UpdateCommitMessage'
	| 'MoveCommit'
	| 'RestoreFromSnapshot'
	| 'ReorderCommit'
	| 'InsertBlankCommit'
	| 'MoveCommitFile'
	| 'FileChanges';

export class Trailer {
	key!: string;
	value!: string;
}

export class SnapshotDiff {
	binary!: boolean;
	@Type(() => RemoteHunk)
	hunks!: RemoteHunk[];
	newPath!: string;
	newSizeBytes!: number;
	oldPath!: string;
	oldSizeBytes!: number;
	skipped!: boolean;
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
	linesAdded!: number;
	linesRemoved!: number;
	filesChanged!: string[];
	details?: SnapshotDetails;
	@Transform((obj) => new Date(obj.value * 1000))
	createdAt!: Date;
}
