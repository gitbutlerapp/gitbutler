export type Style = 'dashed' | 'solid';

export interface CommitNodeData {
	commit: CommitData;
	type?: CellType;
}

export interface Author {
	name?: string;
	email?: string;
	gravatarUrl?: string;
}

/**
 * A minimal set of data required to represent a commit for line drawing purpouses
 */
export interface CommitData {
	id: string;
	title?: string;
	// If an author is not provided, a commit node will not be drawn
	author?: Author;
	relatedRemoteCommit?: CommitData;
	remoteCommitId?: string;
}

export type CellType = 'LocalOnly' | 'LocalAndRemote' | 'Integrated' | 'Remote';
