import type { TreeChange, TreeChanges } from '$lib/hunks/change';
import type { UnifiedDiff } from '$lib/hunks/diff';

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

export type GetWorktreeChangesParams = {
	projectId: string;
};

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

export type UndoCommitParams = {
	projectId: string;
	stackId: string;
	commitOid: string;
};

export function isUndoCommitParams(args: unknown): args is UndoCommitParams {
	return (
		typeof args === 'object' &&
		args !== null &&
		'projectId' in args &&
		typeof args['projectId'] === 'string' &&
		'commitOid' in args &&
		typeof args['commitOid'] === 'string' &&
		'stackId' in args &&
		typeof args['stackId'] === 'string'
	);
}
