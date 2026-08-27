import type {
	CodeViewDiffItem,
	CodeViewItem,
	FileDiffMetadata,
	SelectionSide,
} from "@pierre/diffs";

export type DiffSearchMatch = {
	itemId: string;
	side: SelectionSide;
	lineNumber: number;
	/**
	 * A context line is numbered by the column holding it, and a split diff
	 * holds it twice — this is its number in the deletions column, so marking
	 * can reach both cells. Navigation only ever uses `lineNumber`.
	 */
	deletionsColumnLine?: number;
};

export type DiffSearchSource = {
	fileDiff: FileDiffMetadata;
	isLineRenderable: (lineNumber: number) => boolean;
};

/**
 * Every diff line containing the query, in render order: files as given,
 * hunks top to bottom, deletions before additions within a change block. One
 * match per line and side; context lines match once, on the additions side.
 * Matching is a case-insensitive substring test.
 *
 * The whole model is scanned, not just what the virtualizer has rendered —
 * that is the point: the browser's own find only sees the rendered window.
 */
export const diffSearchMatches = (
	items: Array<CodeViewItem<unknown>>,
	query: string,
	getSource?: (item: CodeViewDiffItem<unknown>) => DiffSearchSource | undefined,
): Array<DiffSearchMatch> => {
	if (query === "") return [];

	const needle = query.toLowerCase();
	const matches: Array<DiffSearchMatch> = [];

	for (const item of items) {
		// Whole-file items (image diffs) have no lines to search.
		if (item.type !== "diff") continue;

		const source = getSource?.(item);
		const { additionLines, deletionLines, hunks } = source?.fileDiff ?? item.fileDiff;
		const lineHasMatch = (text: string | undefined): boolean =>
			text !== undefined && text.toLowerCase().includes(needle);
		const searchExpandedContext = (
			additionStart: number,
			additionEnd: number,
			deletionStart: number,
		): void => {
			if (!source) return;

			for (let lineNumber = additionStart; lineNumber < additionEnd; lineNumber++) {
				if (source.isLineRenderable(lineNumber) && lineHasMatch(additionLines[lineNumber - 1])) {
					matches.push({
						itemId: item.id,
						side: "additions",
						lineNumber,
						deletionsColumnLine: deletionStart + lineNumber - additionStart,
					});
				}
			}
		};
		let previousAdditionEnd = 1;
		let previousDeletionEnd = 1;

		for (const hunk of hunks) {
			searchExpandedContext(previousAdditionEnd, hunk.additionStart, previousDeletionEnd);

			// The content blocks carry indexes into the line arrays but not line
			// numbers, which accumulate from the hunk header as the blocks pass.
			let additionLine = hunk.additionStart;
			let deletionLine = hunk.deletionStart;

			for (const block of hunk.hunkContent) {
				if (block.type === "context") {
					for (let i = 0; i < block.lines; i++) {
						if (lineHasMatch(additionLines[block.additionLineIndex + i])) {
							matches.push({
								itemId: item.id,
								side: "additions",
								lineNumber: additionLine + i,
								deletionsColumnLine: deletionLine + i,
							});
						}
					}
					additionLine += block.lines;
					deletionLine += block.lines;
				} else {
					for (let i = 0; i < block.deletions; i++) {
						if (lineHasMatch(deletionLines[block.deletionLineIndex + i]))
							matches.push({ itemId: item.id, side: "deletions", lineNumber: deletionLine + i });
					}
					deletionLine += block.deletions;

					for (let i = 0; i < block.additions; i++) {
						if (lineHasMatch(additionLines[block.additionLineIndex + i]))
							matches.push({ itemId: item.id, side: "additions", lineNumber: additionLine + i });
					}
					additionLine += block.additions;
				}
			}

			previousAdditionEnd = hunk.additionStart + hunk.additionCount;
			previousDeletionEnd = hunk.deletionStart + hunk.deletionCount;
		}

		searchExpandedContext(previousAdditionEnd, additionLines.length + 1, previousDeletionEnd);
	}

	return matches;
};
