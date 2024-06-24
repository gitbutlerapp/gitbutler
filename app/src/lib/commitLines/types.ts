export type Style =
	| 'local'
	| 'localDashed'
	| 'localAndRemote'
	| 'remote'
	| 'integrated'
	| 'shadow'
	| 'none';

export interface Cell {
	type: 'straight' | 'fork';
	style: Style;
}

export interface CommitNode {
	type: 'small' | 'large';
	commit?: CommitData;
}

export interface BaseNode {}

export interface Line {
	top: Cell;
	bottom: Cell;
	commitNode?: CommitNode;
	baseNode?: BaseNode;
	tallerTop?: boolean;
}

export interface LineGroup {
	// A tuple of two, three, or four lines
	lines: Line[];
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
