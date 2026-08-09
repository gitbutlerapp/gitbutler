import type { LocalAnnotationsByPath } from "#ui/annotation.ts";
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
 * Context is the unchanged code a hunk carries with it. Drawn under the two
 * change lanes so a file reads as code with changes in it, rather than as marks
 * floating in an empty column.
 */
export type MinimapSide = "additions" | "deletions" | "context";

/**
 * A run of lines from one side, in pixels from the top of its file.
 *
 * Widths are per line rather than averaged, so the ruler can resolve individual
 * lines wherever it has the pixels for them, and each carries its leading
 * whitespace separately so indentation can be drawn back from the code.
 * Both are shares of a full lane, in 255ths.
 */
type MinimapMark = {
	top: number;
	height: number;
	side: MinimapSide;
	widths: Uint8Array;
	indents: Uint8Array;
	/** Rendered rows per line, or null when nothing here wraps and each takes one. */
	rows: Uint16Array | null;
	/** Rows the run spans — its line count unless wrapping stretched it. */
	rowCount: number;
};

type RunMetrics = Omit<MinimapMark, "top" | "height" | "side">;

/** The two sides a file line can be numbered on. Context lines are on both. */
type ChangeSide = Exclude<MinimapSide, "context">;

/**
 * A run of consecutive file lines and the pixels it occupies, which is what
 * turns a comment's line number — or a selected range — into a ruler position.
 * Interpolating within the run is exact until wrapping makes lines uneven, and
 * a line or so out is below what the ruler can draw anyway.
 */
type MinimapAnchor = {
	side: ChangeSide;
	start: number;
	lines: number;
	top: number;
	height: number;
};

export type MinimapFile = {
	itemId: string;
	path: string;
	/** Modelled height of the whole file block, used to correct the marks. */
	contentHeight: number;
	marks: Array<MinimapMark>;
	anchors: Array<MinimapAnchor>;
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
	wrapColumns: number | null,
): RunMetrics => {
	const widths = new Uint8Array(lines.length);
	const indents = new Uint8Array(lines.length);
	const rows = wrapColumns === null ? null : new Uint16Array(lines.length);
	let rowCount = 0;

	for (const [index, line] of lines.entries()) {
		const { indent, columns } = lineColumns(line, tabSize);

		widths[index] = share(columns);
		// Never wider than the line it sits in, which also leaves a line that is
		// nothing but whitespace with the two equal — how a blank one is spotted.
		indents[index] = Math.min(share(indent), widths[index]);

		// The viewer breaks on word boundaries where it can, so a line can take one
		// row more than its length demands. Under-counting leaves the rest of the
		// file's own correction to absorb; guessing high would push marks past it.
		const lineRows = wrapColumns === null ? 1 : Math.max(Math.ceil(columns / wrapColumns), 1);
		if (rows) rows[index] = lineRows;
		rowCount += lineRows;
	}

	return { widths, indents, rows, rowCount };
};

const sumRows = (rows: Uint16Array): number => rows.reduce((total, count) => total + count, 0);

/**
 * Split lays each removal beside its addition, so the two share a row and the
 * taller wrap sets its height. Left alone, each side would count only its own
 * rows and the shorter one would drift up the file.
 */
const pairSplitRows = (deletions: RunMetrics, additions: RunMetrics): void => {
	if (!deletions.rows || !additions.rows) return;

	const removed = deletions.rows.length;
	const added = additions.rows.length;
	const paired = new Uint16Array(Math.max(removed, added));

	for (let index = 0; index < paired.length; index++)
		paired[index] = Math.max(deletions.rows[index] ?? 1, additions.rows[index] ?? 1);

	deletions.rows = paired.subarray(0, removed);
	additions.rows = paired.subarray(0, added);
	deletions.rowCount = sumRows(deletions.rows);
	additions.rowCount = sumRows(additions.rows);
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
	wrapColumns,
}: {
	fileParent: FileParent;
	changes: Array<TreeChange>;
	treeChangeDiffs: Array<UnifiedPatch | null>;
	diffStyle: GUISettings["diffStyle"];
	tabSize: number;
	/** Columns a line gets before it wraps, or null when the viewer scrolls instead. */
	wrapColumns: number | null;
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
		const anchors: Array<MinimapAnchor> = [];
		let top = codeViewItemMetrics.diffHeaderHeight;

		for (const [hunkIndex, hunk] of fileDiff.hunks.entries()) {
			if (hunk.collapsedBefore > 0) top += hunkIndex === 0 ? SEPARATOR_LEADING : SEPARATOR_BETWEEN;

			type Placement = { top: number; height: number };

			const mark = (side: MinimapSide, offset: number, metrics: RunMetrics): Placement => {
				const placement = {
					top: top + offset * ROW_HEIGHT,
					height: metrics.rowCount * ROW_HEIGHT,
				};

				marks.push({ ...placement, side, ...metrics });
				return placement;
			};

			const anchor = (
				side: ChangeSide,
				start: number,
				lines: number,
				placement: Placement,
			): void => {
				if (lines > 0) anchors.push({ side, start, lines, ...placement });
			};

			// Rendered rows, and the same walk as if nothing wrapped. Only their
			// difference is taken from this walk — the hunk's own total comes from the
			// viewer, so a part shape we don't recognise can't shift the file.
			let row = 0;
			let unwrapped = 0;
			// The hunk header numbers its first line on each side; every part after
			// that carries the count on from there.
			let additionLine = hunk.additionStart;
			let deletionLine = hunk.deletionStart;

			for (const part of hunk.hunkContent) {
				if (part.type === "context") {
					if (part.lines > 0) {
						// Unchanged, so the two sides hold the same text and either reads it.
						const context = runMetrics(
							fileDiff.additionLines.slice(
								part.additionLineIndex,
								part.additionLineIndex + part.lines,
							),
							tabSize,
							wrapColumns,
						);

						const placement = mark("context", row, context);
						anchor("additions", additionLine, part.lines, placement);
						anchor("deletions", deletionLine, part.lines, placement);
						row += context.rowCount;
					}

					additionLine += part.lines;
					deletionLine += part.lines;
					unwrapped += part.lines;
					continue;
				}

				const deletions =
					part.deletions > 0
						? runMetrics(
								fileDiff.deletionLines.slice(
									part.deletionLineIndex,
									part.deletionLineIndex + part.deletions,
								),
								tabSize,
								wrapColumns,
							)
						: null;
				const additions =
					part.additions > 0
						? runMetrics(
								fileDiff.additionLines.slice(
									part.additionLineIndex,
									part.additionLineIndex + part.additions,
								),
								tabSize,
								wrapColumns,
							)
						: null;

				if (split && deletions && additions) pairSplitRows(deletions, additions);

				if (deletions)
					anchor("deletions", deletionLine, part.deletions, mark("deletions", row, deletions));

				if (additions) {
					// Split puts both sides on the same rows; unified stacks the removals
					// above the additions.
					const offset = split ? row : row + (deletions?.rowCount ?? 0);
					anchor("additions", additionLine, part.additions, mark("additions", offset, additions));
				}

				additionLine += part.additions;
				deletionLine += part.deletions;

				row += split
					? Math.max(deletions?.rowCount ?? 0, additions?.rowCount ?? 0)
					: (deletions?.rowCount ?? 0) + (additions?.rowCount ?? 0);
				unwrapped += split
					? Math.max(part.deletions, part.additions)
					: part.deletions + part.additions;
			}

			const lines = split ? hunk.splitLineCount : hunk.unifiedLineCount;
			top += (lines + (row - unwrapped)) * ROW_HEIGHT;
		}

		return {
			itemId: weakFileIdentityKey({ parent: fileParent, path: change.path }),
			path: change.path,
			contentHeight: top + codeViewItemMetrics.paddingBottom,
			marks,
			anchors,
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
 * One scratch context for measuring text. A fresh canvas per call costs many
 * times the measurement itself and leaves a backing store behind, which matters
 * because the measuring runs on every resize.
 */
let textContext: CanvasRenderingContext2D | null = null;
const measuringContext = (): CanvasRenderingContext2D | null =>
	(textContext ??= document.createElement("canvas").getContext("2d"));

/**
 * Columns a code line gets before the viewer wraps it, read off a rendered one,
 * or null while nothing is rendered yet. The gutter grows with a file's line
 * numbers, so this is the measured file's width and a digit or two out on the
 * rest — worth far less than the wrapping it lets us model at all.
 *
 * The font is monospace, so one character's advance scales to any line length.
 */
export const measureWrapColumns = (viewer: CodeView<Annotation>): number | null => {
	const host = viewer.getContainerElement()?.querySelector("diffs-container");
	const line = host?.shadowRoot?.querySelector("[data-content] > [data-line]");
	if (!line) return null;

	const style = getComputedStyle(line);
	const width =
		line.clientWidth - Number.parseFloat(style.paddingLeft) - Number.parseFloat(style.paddingRight);
	if (!(width > 0)) return null;

	const context = measuringContext();
	if (!context) return null;

	// Built from the longhands rather than taken from `style.font`, which computes
	// to an empty string here — and empty leaves the context on its own default
	// font, quietly measuring something else entirely.
	context.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
	const sample = "0".repeat(50);
	const advance = context.measureText(sample).width / sample.length;

	return advance > 0 ? Math.max(Math.floor(width / advance), 1) : null;
};

/**
 * How far a file's modelled height is from the one CodeView actually laid out.
 * Scaling its marks by this squashes them along with the file when wrapped
 * lines — or a library change to a row or bar — make the model too tall.
 */
export const getMinimapScale = (file: MinimapFile, block: { height: number }): number =>
	file.contentHeight <= 0 ? 0 : block.height / file.contentHeight;

/** Where a file line sits inside its own block, or null if no hunk renders it. */
const lineTop = (file: MinimapFile, side: ChangeSide, line: number): number | null => {
	for (const anchor of file.anchors) {
		if (anchor.side !== side) continue;
		if (line < anchor.start || line >= anchor.start + anchor.lines) continue;

		return anchor.top + ((line - anchor.start) / anchor.lines) * anchor.height;
	}

	return null;
};

/** The range the diff currently has selected, in file line numbers. */
export type MinimapSelection = { itemId: string; side: ChangeSide; start: number; end: number };

export type MinimapOverlays = {
	/** Comment positions, in scroll-content pixels. */
	pins: Array<number>;
	band: { top: number; height: number } | null;
};

/**
 * Comment pins and the selected range, in the same scroll-content pixels as the
 * geometry — resolved here rather than in the painter, which knows about ink
 * and not about which line a comment hangs off.
 */
export const getMinimapOverlays = ({
	files,
	geometry,
	annotationsByPath,
	selection,
}: {
	files: Array<MinimapFile>;
	geometry: MinimapGeometry;
	annotationsByPath: LocalAnnotationsByPath;
	selection: MinimapSelection | null;
}): MinimapOverlays => {
	const pins: Array<number> = [];
	let band: { top: number; height: number } | null = null;

	for (const [index, file] of files.entries()) {
		const block = geometry.blocks[index];
		if (!block) continue;

		const scale = getMinimapScale(file, block);
		const place = (side: ChangeSide, line: number): number | null => {
			const local = lineTop(file, side, line);
			return local === null ? null : block.top + local * scale;
		};

		for (const annotation of annotationsByPath.get(file.path) ?? []) {
			const top = place(annotation.side, annotation.lineNumber);
			if (top !== null) pins.push(top);
		}

		if (selection?.itemId !== file.itemId) continue;

		// A hunk that only removes lines carries no addition to number the range
		// against, and the other way round, so fall through to whichever side has it.
		const other = selection.side === "additions" ? "deletions" : "additions";
		const locate = (line: number): number | null =>
			place(selection.side, line) ?? place(other, line);

		const start = locate(selection.start);
		// The band should stop where the line after the range starts; a selection
		// running to the end of a hunk has no such line to ask for.
		const end = locate(selection.end + 1) ?? locate(selection.end);
		if (start !== null) band = { top: start, height: Math.max((end ?? start) - start, 0) };
	}

	return { pins, band };
};

/**
 * The visible window: how much of the content it covers, and how far through
 * the scrollable range it sits.
 *
 * Progress rather than a top, because the lens is held to a minimum height and
 * one wider than the window it stands for travels a track shortened by the
 * difference — the arithmetic every scrollbar does. Where the lens is its true
 * size the two agree exactly, so only a diff long enough to floor it moves.
 */
export const getMinimapViewport = (
	viewer: CodeView<Annotation>,
): { height: number; progress: number } => {
	const total = contentHeight(viewer);
	if (total <= 0) return { height: 1, progress: 0 };

	const scrollable = scrollableHeight(viewer);
	const progress = scrollable <= 0 ? 0 : viewer.getScrollTop() / scrollable;

	return { height: viewer.getHeight() / total, progress: Math.min(Math.max(progress, 0), 1) };
};

/** How far the diff can scroll: everything below the window it doesn't fill. */
const scrollableHeight = (viewer: CodeView<Annotation>): number =>
	Math.max(contentHeight(viewer) - viewer.getHeight(), 0);

/** Scroll `progress` of the way through the range, 0 being the top and 1 the end. */
export const scrollMinimapTo = (viewer: CodeView<Annotation>, progress: number): void => {
	viewer.scrollTo({
		type: "position",
		// CodeView lifts a position target by the sticky header, so the row you
		// asked for isn't left under it. The minimap is placing the viewport rather
		// than a row, so add it back or every drag sits a header too high.
		position: progress * scrollableHeight(viewer) + codeViewItemMetrics.diffHeaderHeight,
		behavior: "instant",
	});
};
