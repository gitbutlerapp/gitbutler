/**
 * How each level's guide draws on a tree's row, outermost first; a level left out is a `line`.
 * `line` runs on through the row; `last`, on the last row under its parent, turns into the row's
 * glyph and ends there; `none` on the rows under a `last` one, where that level has ended.
 *
 * A row on a chain, a stack's branch or a commit, sits on its parent's guide as a node rather
 * than hanging off it: `node` puts its mark on the line, which runs on through, and `nodeLast`
 * ends the line at the last one's mark. The line says the rows follow on from each other, where
 * `last`'s tail says they sit side by side.
 */
export type Guides = ReadonlyArray<"line" | "last" | "none" | "node" | "nodeLast">;

/**
 * Each row's guides, worked out from the rows of a tree in the order they draw: how deep each sits
 * (`depth`, as the row takes it). A row can't see the rows after it, so a host works its guides
 * out here, from the rows it shows, rather than by hand. `chained` puts a row on its parent's line
 * as a node: a stack's branches and a branch's commits, which build on each other.
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
					? chained
						? "nodeLast"
						: "last"
					: chained
						? "node"
						: "line",
		);
		ancestorLast.length = depth;
		ancestorLast[depth] = isLast;
		return guides;
	});
};
