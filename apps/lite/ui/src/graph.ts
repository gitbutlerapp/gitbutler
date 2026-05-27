/**
 * @file
 *
 * How we render graphs. See also {@link https://docs.rs/sapling-renderdag/latest/renderdag/}.
 */

import type { GraphRow, LinkLine, NodeLine, PadLine } from "@gitbutler/but-sdk";
import * as linkLine from "./link-line.ts";

export const graphGlyphs = {
	ancestor: "╷ ",
	branch: "◎ ",
	commit: "● ",
	forkBoth: "┬─",
	forkLeft: "╮ ",
	forkRight: "╭─",
	horizontal: "──",
	joinBoth: "┼─",
	joinLeft: "┤ ",
	joinRight: "├─",
	mergeBoth: "┴─",
	mergeLeft: "╯ ",
	mergeRight: "╰─",
	parent: "│ ",
	space: "  ",
	termination: "~ ",
} as const;

export const nodeCellGlyph = ({ row, line }: { row: GraphRow; line: NodeLine }): string => {
	switch (line) {
		case "Node":
			return row.data.startsWith("refs/") ? graphGlyphs.branch : graphGlyphs.commit;
		case "Parent":
			return graphGlyphs.parent;
		case "Ancestor":
			return graphGlyphs.ancestor;
		case "Blank":
			return graphGlyphs.space;
	}
};

export const padCellGlyph = (line: PadLine): string => {
	switch (line) {
		case "Parent":
			return graphGlyphs.parent;
		case "Ancestor":
			return graphGlyphs.ancestor;
		case "Blank":
			return graphGlyphs.space;
	}
};

export const linkCellGlyph = (line: LinkLine): string => {
	if (linkLine.intersects(line, linkLine.HORIZONTAL)) {
		if (
			linkLine.intersects(line, linkLine.CHILD) ||
			(linkLine.intersects(line, linkLine.ANY_FORK) &&
				linkLine.intersects(line, linkLine.ANY_MERGE)) ||
			(linkLine.intersects(line, linkLine.ANY_FORK) &&
				linkLine.intersects(line, linkLine.VERT_PARENT))
		)
			return graphGlyphs.joinBoth;
		if (linkLine.intersects(line, linkLine.ANY_FORK)) return graphGlyphs.forkBoth;
		if (linkLine.intersects(line, linkLine.ANY_MERGE)) return graphGlyphs.mergeBoth;
		return graphGlyphs.horizontal;
	}

	if (linkLine.intersects(line, linkLine.VERT_PARENT)) {
		const left = linkLine.intersects(line, linkLine.LEFT_MERGE | linkLine.LEFT_FORK);
		const right = linkLine.intersects(line, linkLine.RIGHT_MERGE | linkLine.RIGHT_FORK);
		if (left && right) return graphGlyphs.joinBoth;
		if (left) return graphGlyphs.joinLeft;
		if (right) return graphGlyphs.joinRight;
		return graphGlyphs.parent;
	}

	if (
		linkLine.intersects(line, linkLine.VERTICAL) &&
		!linkLine.intersects(line, linkLine.LEFT_FORK | linkLine.RIGHT_FORK)
	) {
		const left = linkLine.intersects(line, linkLine.LEFT_MERGE);
		const right = linkLine.intersects(line, linkLine.RIGHT_MERGE);
		if (left && right) return graphGlyphs.joinBoth;
		if (left) return graphGlyphs.joinLeft;
		if (right) return graphGlyphs.joinRight;
		if (linkLine.intersects(line, linkLine.VERT_ANCESTOR)) return graphGlyphs.ancestor;
		return graphGlyphs.parent;
	}

	if (
		linkLine.intersects(line, linkLine.LEFT_FORK) &&
		linkLine.intersects(line, linkLine.LEFT_MERGE | linkLine.CHILD)
	)
		return graphGlyphs.joinLeft;
	if (
		linkLine.intersects(line, linkLine.RIGHT_FORK) &&
		linkLine.intersects(line, linkLine.RIGHT_MERGE | linkLine.CHILD)
	)
		return graphGlyphs.joinRight;
	if (
		linkLine.intersects(line, linkLine.LEFT_MERGE) &&
		linkLine.intersects(line, linkLine.RIGHT_MERGE)
	)
		return graphGlyphs.mergeBoth;
	if (
		linkLine.intersects(line, linkLine.LEFT_FORK) &&
		linkLine.intersects(line, linkLine.RIGHT_FORK)
	)
		return graphGlyphs.forkBoth;
	if (linkLine.intersects(line, linkLine.LEFT_FORK)) return graphGlyphs.forkLeft;
	if (linkLine.intersects(line, linkLine.LEFT_MERGE)) return graphGlyphs.mergeLeft;
	if (linkLine.intersects(line, linkLine.RIGHT_FORK)) return graphGlyphs.forkRight;
	if (linkLine.intersects(line, linkLine.RIGHT_MERGE)) return graphGlyphs.mergeRight;
	return graphGlyphs.space;
};
