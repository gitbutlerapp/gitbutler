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

export interface Line {
	top: Cell;
	bottom: Cell;
	node?: CommitNode;
}

export interface LineGroup {
	// A tuple of two, three, or four lines
	lines: [Line, Line] | [Line, Line, Line] | [Line, Line, Line, Line];
}

export interface Author {
	email?: string;
	gravatarUrl?: URL;
}

/**
 * A minimal set of data required to represent a commit for line drawing purpouses
 */
export interface CommitData {
	id: string;
	author: Author;
	relatedRemoteCommit?: CommitData;
}
