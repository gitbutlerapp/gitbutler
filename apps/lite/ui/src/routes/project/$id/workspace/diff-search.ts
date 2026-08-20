import type { CodeViewItem, SelectionSide } from "@pierre/diffs";

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
): Array<DiffSearchMatch> => {
	if (query === "") return [];

	const needle = query.toLowerCase();
	const matches: Array<DiffSearchMatch> = [];

	for (const item of items) {
		// Whole-file items (image diffs) have no lines to search.
		if (item.type !== "diff") continue;

		const { additionLines, deletionLines, hunks } = item.fileDiff;
		const lineHasMatch = (text: string | undefined): boolean =>
			text !== undefined && text.toLowerCase().includes(needle);

		for (const hunk of hunks) {
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
		}
	}

	return matches;
};
