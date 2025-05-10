import { HunkSection, type ContentSection } from '$lib/utils/fileSections';
import type { AnyFile } from '$lib/files/file';

export function computeAddedRemovedByFiles(...files: AnyFile[]) {
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

export function computeAddedRemovedByHunk(section: HunkSection | ContentSection): {
	added: any;
	removed: any;
} {
	if (section instanceof HunkSection) {
		const lines = section.hunk.diff.split('\n');
		return {
			added: lines.filter((l) => l.startsWith('+')).length,
			removed: lines.filter((l) => l.startsWith('-')).length
		};
	}
	return {
		added: 0,
		removed: 0
	};
}
