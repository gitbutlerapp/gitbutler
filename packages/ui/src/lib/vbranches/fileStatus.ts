import type { File } from './types';

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

export function computeFileStatus(file: File) {
	const { added, removed } = computedAddedRemoved(file);
	// TODO: How should we handle this for binary files? See: https://git-scm.com/docs/git-status
	const contentLineCount = file.content?.trim().split('\n').length;
	return added == contentLineCount ? 'A' : removed == contentLineCount ? 'D' : 'M';
}
