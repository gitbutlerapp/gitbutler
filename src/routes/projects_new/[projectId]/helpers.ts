import type { Branch, Commit, File, Hunk } from './types';

let fileCounter = 0;
let commitCounter = 0;
let branchCounter = 0;

export function createFile(args: { hunks: [Hunk]; filePath: string; isShadow: boolean }): File {
	fileCounter++;
	return {
		id: `file-${fileCounter}`,
		path: args.filePath,
		kind: 'file',
		hunks: args.hunks,
		isDndShadowItem: args.isShadow
	};
}

export function createCommit(args: { files: File[]; isShadow: boolean }): Commit {
	commitCounter++;
	return {
		id: `commit-${commitCounter}`,
		description: `New commit # ${commitCounter}`,
		kind: 'commit',
		files: args.files,
		isDndShadowItem: args.isShadow
	};
}

export function createBranch(args: { commits: Commit[] }): Branch {
	branchCounter++;
	return {
		id: `branch-${branchCounter}`,
		name: `new branch ${branchCounter}`,
		active: true,
		kind: 'branch',
		commits: args.commits
	};
}
