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

export interface BaseNodeData {}

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
	gravatarUrl?: URL;
}

/**
 * A minimal set of data required to represent a commit for line drawing purpouses
 */
export interface CommitData {
	id: string;
	title?: string;
	author: Author;
	relatedRemoteCommit?: CommitData;
}
