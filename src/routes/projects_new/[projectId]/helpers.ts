import { Branch, Commit, File, type Hunk } from './types';
import { plainToInstance } from 'class-transformer';

let fileCounter = 0;
let commitCounter = 0;
let branchCounter = 0;

export function createFile(path: string, hunk: Hunk): File {
	fileCounter++;
	return plainToInstance(File, {
		id: `file-${fileCounter}`,
		path: path,
		kind: 'file',
		hunks: [hunk]
	});
}

export function createCommit(file: File): Commit {
	commitCounter++;
	return plainToInstance(Commit, {
		id: `commit-${commitCounter}`,
		description: `New commit # ${commitCounter}`,
		kind: 'commit',
		files: [file]
	});
}

export function createBranch(commit: Commit): Branch {
	branchCounter++;
	return plainToInstance(Branch, {
		id: `branch-${branchCounter}`,
		name: `new branch ${branchCounter}`,
		active: true,
		kind: 'branch',
		commits: [commit]
	});
}
