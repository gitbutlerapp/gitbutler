import { Branch, File, type Hunk } from './types';
import { plainToInstance } from 'class-transformer';

let branchCounter = 0;

export function createFile(path: string, hunk: Hunk): File {
	return plainToInstance(File, {
		id: path,
		path: path,
		hunks: [hunk]
	});
}

export function createBranch(file: File): Branch {
	branchCounter++;
	return plainToInstance(Branch, {
		id: `branch-${branchCounter}`,
		name: `new branch ${branchCounter}`,
		active: true,
		files: [file]
	});
}
