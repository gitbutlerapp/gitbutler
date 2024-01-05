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
		const diff = file.hunks[0].diff;
		if (/^@@ -0,0 /.test(diff)) return 'A';
		if (/^@@ -\d+,\d+ \+0,0/.test(diff)) return 'D';
	}
	return 'M';
}
