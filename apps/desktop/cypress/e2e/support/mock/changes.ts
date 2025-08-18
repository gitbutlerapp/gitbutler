import type { TreeChange, TreeChanges } from '$lib/hunks/change';
import type { UnifiedDiff } from '$lib/hunks/diff';
import type { DiffHunk } from '$lib/hunks/hunk';

export const MOCK_TREE_CHANGES: TreeChanges = {
	changes: [],
	stats: {
		linesAdded: 0,
		linesRemoved: 0,
		filesChanged: 0
	}
};

export function strToBytes(str: string): number[] {
	const bytes: number[] = [];
	for (let i = 0; i < str.length; i++) {
		bytes.push(str.charCodeAt(i));
	}
	return bytes;
}

export function bytesToStr(bytes: number[]): string {
	const str: string[] = [];
	for (let i = 0; i < bytes.length; i++) {
		const byte = bytes[i]!;
		if (byte === 0) {
			break;
		}
		str.push(String.fromCharCode(byte));
	}
	return str.join('');
}

export const MOCK_TREE_CHANGE_A: TreeChange = {
	path: '/path/to/projectA/fileA.txt',
	pathBytes: strToBytes('/path/to/projectA/fileA.txt'),
	status: {
		type: 'Addition',
		subject: {
			state: {
				id: 'addition-id',
				kind: 'addition'
			},
			isUntracked: true
		}
	}
};

const MOCK_FILE_ADDITION_DIFF: string = `@@ -0,0 +1,3 @@
+Line 1
+Line 2
+Line 3`;

export const MOCK_UNIFIED_DIFF: UnifiedDiff = {
	type: 'Patch',
	subject: {
		isResultOfBinaryToTextConversion: false,
		linesAdded: 3,
		linesRemoved: 0,
		hunks: [{ oldStart: 0, oldLines: 0, newStart: 1, newLines: 3, diff: MOCK_FILE_ADDITION_DIFF }]
	}
};

export function createMockUnifiedDiffPatch(
	hunks: DiffHunk[],
	linesAdded: number,
	linesRemoved: number
): UnifiedDiff {
	return {
		type: 'Patch',
		subject: {
			isResultOfBinaryToTextConversion: false,
			linesAdded,
			linesRemoved,
			hunks
		}
	};
}

export type GetWorktreeChangesParams = {
	projectId: string;
};

export function hasProjectId(args: unknown): args is { projectId: string } {
	return (
		typeof args === 'object' &&
		args !== null &&
		'projectId' in args &&
		typeof args['projectId'] === 'string'
	);
}

export function isGetWorktreeChangesParams(args: unknown): args is GetWorktreeChangesParams {
	return (
		typeof args === 'object' &&
		args !== null &&
		'projectId' in args &&
		typeof args['projectId'] === 'string'
	);
}

export type GetDiffParams = {
	projectId: string;
	change: TreeChange;
};

export function isTreeChange(args: unknown): args is TreeChange {
	return (
		typeof args === 'object' &&
		args !== null &&
		'path' in args &&
		typeof args['path'] === 'string' &&
		'status' in args &&
		typeof args['status'] === 'object' &&
		'pathBytes' in args &&
		Array.isArray(args['pathBytes']) &&
		args['pathBytes'].every((byte) => typeof byte === 'number')
	);
}

export function isGetDiffParams(args: unknown): args is GetDiffParams {
	return (
		typeof args === 'object' &&
		args !== null &&
		'projectId' in args &&
		typeof args['projectId'] === 'string' &&
		'change' in args &&
		isTreeChange(args['change'])
	);
}

export type GetCommitChangesParams = {
	projectId: string;
	commitId: string;
};

export function isGetCommitChangesParams(args: unknown): args is GetCommitChangesParams {
	return (
		typeof args === 'object' &&
		args !== null &&
		'projectId' in args &&
		typeof args['projectId'] === 'string' &&
		'commitId' in args &&
		typeof args['commitId'] === 'string'
	);
}

export type GetBranchChangesParams = {
	projectId: string;
	stackId?: string;
	branch: string;
};

export function isGetBranchChangesParams(args: unknown): args is GetBranchChangesParams {
	return (
		typeof args === 'object' &&
		args !== null &&
		'projectId' in args &&
		typeof args['projectId'] === 'string' &&
		(typeof (args as any).stackId === 'string' || (args as any).stackId === undefined) &&
		'branch' in args &&
		typeof args['branch'] === 'string'
	);
}

export type UndoCommitParams = {
	projectId: string;
	stackId: string;
	commitId: string;
};

export function isUndoCommitParams(args: unknown): args is UndoCommitParams {
	return (
		typeof args === 'object' &&
		args !== null &&
		'projectId' in args &&
		typeof args['projectId'] === 'string' &&
		'commitId' in args &&
		typeof args['commitId'] === 'string' &&
		'stackId' in args &&
		typeof args['stackId'] === 'string'
	);
}

export const MOCK_TREE_CHANGE_ADDITION: TreeChange = {
	path: '/mock/addition.txt',
	pathBytes: strToBytes('/mock/addition.txt'),
	status: {
		type: 'Addition',
		subject: {
			state: { id: 'addition-id', kind: 'addition' },
			isUntracked: false
		}
	}
};

export function createMockAdditionTreeChange(props: Partial<TreeChange>): TreeChange {
	const path = props.path ?? MOCK_TREE_CHANGE_ADDITION.path;
	const pathBytes = props.pathBytes ?? strToBytes(path);
	return {
		...MOCK_TREE_CHANGE_ADDITION,
		...props,
		path,
		pathBytes
	};
}

export const MOCK_TREE_CHANGE_DELETION: TreeChange = {
	path: '/mock/deletion.txt',
	pathBytes: strToBytes('/mock/deletion.txt'),
	status: {
		type: 'Deletion',
		subject: {
			previousState: { id: 'deletion-prev-id', kind: 'deletion' }
		}
	}
};

export function createMockDeletionTreeChange(props: Partial<TreeChange>): TreeChange {
	const path = props.path ?? MOCK_TREE_CHANGE_DELETION.path;
	const pathBytes = props.pathBytes ?? strToBytes(path);
	return {
		...MOCK_TREE_CHANGE_DELETION,
		...props,
		path,
		pathBytes
	};
}

export const MOCK_TREE_CHANGE_MODIFICATION: TreeChange = {
	path: '/mock/modification.txt',
	pathBytes: strToBytes('/mock/modification.txt'),
	status: {
		type: 'Modification',
		subject: {
			previousState: { id: 'mod-prev-id', kind: 'mod-prev' },
			state: { id: 'mod-id', kind: 'mod' },
			flags: null
		}
	}
};

export function createMockModificationTreeChange(props: Partial<TreeChange>): TreeChange {
	const path = props.path ?? MOCK_TREE_CHANGE_MODIFICATION.path;
	const pathBytes = props.pathBytes ?? strToBytes(path);
	return {
		...MOCK_TREE_CHANGE_MODIFICATION,
		...props,
		path,
		pathBytes
	};
}

export const MOCK_TREE_CHANGE_RENAME: TreeChange = {
	path: '/mock/renamed.txt',
	pathBytes: strToBytes('/mock/renamed.txt'),
	status: {
		type: 'Rename',
		subject: {
			previousPath: '/mock/oldname.txt',
			previousPathBytes: strToBytes('/mock/oldname.txt'),
			previousState: { id: 'rename-prev-id', kind: 'rename-prev' },
			state: { id: 'rename-id', kind: 'rename' },
			flags: null
		}
	}
};

export function createMockRenameTreeChange(props: Partial<TreeChange>): TreeChange {
	const path = props.path ?? MOCK_TREE_CHANGE_RENAME.path;
	const pathBytes = props.pathBytes ?? strToBytes(path);
	return {
		...MOCK_TREE_CHANGE_RENAME,
		...props,
		path,
		pathBytes
	};
}
