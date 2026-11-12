/**
 * How each level's guide draws on a tree's row, outermost first; a level left out is a `line`.
 * `line` runs on through the row; `last`, on the last row under its parent, turns into the row's
 * glyph and ends there; `none` on the rows under a `last` one, where that level has ended. `stub`,
 * on the row's own level, runs on and reaches toward its glyph: for a git object, a stack, a branch
 * or a commit, where `line` is for rows that only group others.
 */
export type Guides = ReadonlyArray<"line" | "stub" | "last" | "none">;

/**
 * Each row's guides, worked out from the rows of a tree in the order they draw: how deep each sits
 * (`depth`, as the row takes it) and whether it is a git object (`chained`: a stack, a branch, a
 * commit), which meets its guide with a stub unless it is last. A row can't see the rows after it,
 * so a host works its guides out here, from the rows it shows, rather than by hand.
 *
 * @import import { treeGuides } from "@gitbutler/but-ui-react/treeGuides.ts";
 */
export const treeGuides = (
	rows: ReadonlyArray<{ depth: number; chained?: boolean }>,
): Array<Guides> => {
	// From the end: a row is last when no row at its depth follows before a shallower one.
	const last: Array<boolean> = [];
	const followed: Array<boolean> = [];
	for (let index = rows.length - 1; index >= 0; index--) {
		const { depth } = rows[index] ?? { depth: 0 };
		last[index] = followed[depth] !== true;
		followed.length = depth;
		followed[depth] = true;
	}
	// From the start: a level has ended under a row whose ancestor at the next depth was last.
	const ancestorLast: Array<boolean> = [];
	return rows.map(({ depth, chained = false }, index) => {
		const isLast = last[index] === true;
		const guides = Array.from({ length: depth }, (_, level) =>
			level < depth - 1
				? ancestorLast[level + 1] === true
					? "none"
					: "line"
				: isLast
					? "last"
					: chained
						? "stub"
						: "line",
		);
		ancestorLast.length = depth;
		ancestorLast[depth] = isLast;
		return guides;
	});
};
