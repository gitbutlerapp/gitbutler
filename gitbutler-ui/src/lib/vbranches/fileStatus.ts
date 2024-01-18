import type { File } from './types';

export type FileStatus = 'A' | 'M' | 'D';

export function computedAddedRemoved(...files: File[]) {
	return files
		.flatMap((f) => f.hunks)
		.map((h) => h.diff.split('\n'))
		.reduce(
			(acc, lines) => ({
				added: acc.added + lines.filter((l) => l.startsWith('+')).length,
				removed: acc.removed + lines.filter((l) => l.startsWith('-')).length
			}),
			{ added: 0, removed: 0 }
		);
}

export function computeFileStatus(file: File): FileStatus {
	if (file.hunks.length == 1) {
		const changeType = file.hunks[0].changeType;
		if (changeType == 'added') {
			return 'A';
		} else if (changeType == 'deleted') {
			return 'D';
		}
	}
	return 'M';
}
