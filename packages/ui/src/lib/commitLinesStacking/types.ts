export type Style = 'dashed' | 'solid';
export interface CellData {
	type: CellType;
	style?: Style;
}

export interface CommitNodeData {
	// line: LineData;
	commit: CommitData;
	type?: CellType;
}

export interface LineData {
	top: CellData;
	bottom: CellData;
	commitNode?: CommitNodeData;
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
}

export type CellType = 'Local' | 'Remote' | 'Upstream' | 'LocalShadow';
