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
