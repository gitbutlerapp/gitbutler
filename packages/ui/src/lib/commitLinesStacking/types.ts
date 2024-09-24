export type Color = 'local' | 'localAndRemote' | 'remote' | 'integrated' | 'shadow' | 'none';

export type Style = 'dashed';
export interface CellData {
	color: Color;
	style?: Style;
}

export interface CommitNodeData {
	line: LineData;
	type?: CellType;
	commit?: CommitData;
}

export interface LineData {
	top: CellData;
	bottom: CellData;
	commitNode?: CommitNodeData;
}

// export interface LineGroupData {
// 	// A tuple of two, three, or four lines
// 	lines: LineData[];
// }

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
