import { weakFileIdentityKey, type FileParent } from "#ui/operands.ts";
import type { GUISettings } from "#electron/settings.ts";
import type { TreeChange, UnifiedPatch } from "@gitbutler/but-sdk";
import type { CodeView } from "@pierre/diffs";
import {
	type Annotation,
	codeViewItemMetrics,
	codeViewLayout,
	parseChangeDiff,
} from "./diff-view.ts";

/**
 * The diff viewer's own row height and separator-bar costs, measured against
 * its exact `getLinePosition`. Unlike the layout and metrics we hand CodeView,
 * these are the library's — hence the correction in `getMinimapScale`. The
 * leading bar sits directly under the file header and so has spacing one side.
 */
const ROW_HEIGHT = 20;
const SEPARATOR_LEADING = 40;
const SEPARATOR_BETWEEN = 48;

/**
 * Changed lines the map will model before it gives up and lets the native
 * scrollbar take over. The work here is linear in those lines, and past this a
 * ruler costs more to build than it can usefully show. Counted off the patch,
 * so a runaway change set is turned away without parsing any of it.
 */
const MAX_CHANGED_LINES = 250_000;

/**
 * How many viewports tall the diff has to be before the map earns its space.
 * Just over one, and the marker covers most of the ruler and says nothing the
 * scrollbar wouldn't.
 */
const MIN_VIEWPORTS = 2;

/** Line width at which a mark fills its lane. */
const REFERENCE_COLUMNS = 100;

/**
 * A run of added or removed lines, in pixels from the top of its file.
 *
 * Widths are per line rather than averaged, so the ruler can resolve individual
 * lines wherever it has the pixels for them, and each carries its leading
 * whitespace separately so indentation can be drawn back from the code.
 * Both are shares of a full lane, in 255ths.
 */
type MinimapMark = {
	top: number;
	height: number;
	side: "additions" | "deletions";
	widths: Uint8Array;
	indents: Uint8Array;
};

export type MinimapFile = {
	itemId: string;
	path: string;
	/** Modelled height of the whole file block, used to correct the marks. */
	contentHeight: number;
	marks: Array<MinimapMark>;
};

/** Scroll-content pixels, the space every position here is measured in. */
export type MinimapGeometry = {
	contentHeight: number;
	blocks: Array<{ top: number; height: number }>;
};

/** CodeView reports scroll height without the layout's outer padding; tops include it. */
const contentHeight = (viewer: CodeView<Annotation>): number =>
	codeViewLayout.paddingTop + viewer.getScrollHeight() + codeViewLayout.paddingBottom;

/**
 * Rendered width of a line in columns, and the leading whitespace within it.
 * Only the indent is expanded to tab stops — tabs inside a line are rare enough
 * not to justify scanning every character of every line in the diff.
 */
const lineColumns = (line: string, tabSize: number): { indent: number; columns: number } => {
	let end = line.length;
	while (end > 0 && (line[end - 1] === "\n" || line[end - 1] === "\r")) end--;

	let indent = 0;
	let index = 0;
	while (index < end && (line[index] === "\t" || line[index] === " ")) {
		indent += line[index] === "\t" ? tabSize - (indent % tabSize) : 1;
		index++;
	}

	return { indent, columns: indent + (end - index) };
};

const share = (columns: number): number =>
	Math.round(Math.min(columns / REFERENCE_COLUMNS, 1) * 255);

const runMetrics = (
	lines: Array<string>,
	tabSize: number,
): { widths: Uint8Array; indents: Uint8Array } => {
	const widths = new Uint8Array(lines.length);
	const indents = new Uint8Array(lines.length);

	for (const [index, line] of lines.entries()) {
		const { indent, columns } = lineColumns(line, tabSize);

		widths[index] = share(columns);
		// Never wider than the line it sits in, which also leaves a line that is
		// nothing but whitespace with the two equal — how a blank one is spotted.
		indents[index] = Math.min(share(indent), widths[index]);
	}

	return { widths, indents };
};

/**
 * Where each file's changes sit inside its own block.
 *
 * Only hunks are rendered — the lines between them are collapsed behind a
 * separator bar — so a change's offset is a sum of whole rows and bars rather
 * than a share of the file's line count.
 *
 * Item IDs are derived the same way as in `getDiffView` rather than read back
 * off it, so the minimap doesn't inherit that view's selection-driven churn.
 */
export const getMinimapFiles = ({
	fileParent,
	changes,
	treeChangeDiffs,
	diffStyle,
	tabSize,
}: {
	fileParent: FileParent;
	changes: Array<TreeChange>;
	treeChangeDiffs: Array<UnifiedPatch | null>;
	diffStyle: GUISettings["diffStyle"];
	tabSize: number;
}): Array<MinimapFile> => {
	const changedLines = treeChangeDiffs.reduce(
		(total, diff) =>
			total + (diff?.type === "Patch" ? diff.subject.linesAdded + diff.subject.linesRemoved : 0),
		0,
	);
	if (changedLines > MAX_CHANGED_LINES) return [];

	const split = diffStyle === "split";

	return changes.map((change, changeIndex) => {
		const { fileDiff } = parseChangeDiff(change, treeChangeDiffs[changeIndex] ?? null);
		const marks: Array<MinimapMark> = [];
		let top = codeViewItemMetrics.diffHeaderHeight;

		for (const [hunkIndex, hunk] of fileDiff.hunks.entries()) {
			if (hunk.collapsedBefore > 0) top += hunkIndex === 0 ? SEPARATOR_LEADING : SEPARATOR_BETWEEN;

			let row = 0;
			for (const part of hunk.hunkContent) {
				if (part.type === "context") {
					row += part.lines;
					continue;
				}

				if (part.deletions > 0) {
					marks.push({
						top: top + row * ROW_HEIGHT,
						height: part.deletions * ROW_HEIGHT,
						side: "deletions",
						...runMetrics(
							fileDiff.deletionLines.slice(
								part.deletionLineIndex,
								part.deletionLineIndex + part.deletions,
							),
							tabSize,
						),
					});
				}
				if (part.additions > 0) {
					marks.push({
						// Split puts both sides on the same rows; unified stacks the
						// removals above the additions.
						top: top + (split ? row : row + part.deletions) * ROW_HEIGHT,
						height: part.additions * ROW_HEIGHT,
						side: "additions",
						...runMetrics(
							fileDiff.additionLines.slice(
								part.additionLineIndex,
								part.additionLineIndex + part.additions,
							),
							tabSize,
						),
					});
				}

				row += split ? Math.max(part.deletions, part.additions) : part.deletions + part.additions;
			}

			top += (split ? hunk.splitLineCount : hunk.unifiedLineCount) * ROW_HEIGHT;
		}

		return {
			itemId: weakFileIdentityKey({ parent: fileParent, path: change.path }),
			path: change.path,
			contentHeight: top + codeViewItemMetrics.paddingBottom,
			marks,
		};
	});
};

/**
 * File positions, or null when the diff is too short to be worth mapping.
 *
 * Tops are exact rather than sampled: CodeView lays out every item, including
 * ones virtualization hasn't rendered. Heights are the distance to the next
 * file less the inter-item gap, since CodeView exposes no per-item height.
 */
export const getMinimapGeometry = (
	viewer: CodeView<Annotation>,
	itemIds: Array<string>,
): MinimapGeometry | null => {
	const total = contentHeight(viewer);
	if (itemIds.length === 0 || total < viewer.getHeight() * MIN_VIEWPORTS) return null;

	const blocks: Array<{ top: number; height: number }> = [];

	for (const [index, itemId] of itemIds.entries()) {
		const top = viewer.getTopForItem(itemId);
		// CodeView can hold a stale item list for a frame after the diff changes.
		if (top === undefined) return null;

		const nextId = itemIds[index + 1];
		const nextTop = nextId === undefined ? undefined : viewer.getTopForItem(nextId);
		if (nextId !== undefined && nextTop === undefined) return null;

		const bottom =
			nextTop === undefined ? total - codeViewLayout.paddingBottom : nextTop - codeViewLayout.gap;

		blocks.push({ top, height: Math.max(bottom - top, 0) });
	}

	return { contentHeight: total, blocks };
};

/**
 * How far a file's modelled height is from the one CodeView actually laid out.
 * Scaling its marks by this squashes them along with the file when wrapped
 * lines — or a library change to a row or bar — make the model too tall.
 */
export const getMinimapScale = (file: MinimapFile, block: { height: number }): number =>
	file.contentHeight <= 0 ? 0 : block.height / file.contentHeight;

/** The visible window, as fractions of the content. */
export const getMinimapViewport = (
	viewer: CodeView<Annotation>,
): { top: number; height: number } => {
	const total = contentHeight(viewer);
	if (total <= 0) return { top: 0, height: 1 };

	return { top: viewer.getScrollTop() / total, height: viewer.getHeight() / total };
};

/** Scroll so the top of the viewport sits at `fraction` of the content. */
export const scrollMinimapTo = (viewer: CodeView<Annotation>, fraction: number): void => {
	viewer.scrollTo({
		type: "position",
		// CodeView lifts a position target by the sticky header, so the row you
		// asked for isn't left under it. The minimap is placing the viewport rather
		// than a row, so add it back or every drag sits a header too high.
		position: fraction * contentHeight(viewer) + codeViewItemMetrics.diffHeaderHeight,
		behavior: "instant",
	});
};
