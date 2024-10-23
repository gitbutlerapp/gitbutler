export type Color = 'local' | 'localAndRemote' | 'remote' | 'integrated' | 'shadow' | 'none';

export type Style = 'dashed';
export interface CellData {
	type: 'straight' | 'fork';
	color: Color;
	style?: Style;
}

export interface CommitNodeData {
	type: 'small' | 'large';
	commit?: CommitData;
}

export type BaseNodeData = object;

export interface LineData {
	top: CellData;
	bottom: CellData;
	commitNode?: CommitNodeData;
	baseNode?: BaseNodeData;
}

export interface LineGroupData {
	// A tuple of two, three, or four lines
	lines: LineData[];
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
