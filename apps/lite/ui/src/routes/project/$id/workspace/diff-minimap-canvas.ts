import type { GUISettings } from "#electron/settings.ts";
import {
	getMinimapScale,
	type MinimapFile,
	type MinimapGeometry,
	type MinimapLayout,
	type MinimapOverlays,
	type MinimapSide,
} from "./diff-minimap.ts";

/** The channel between the two lanes in side-by-side. */
const LANE_SPLIT = 1;

/** Vertical room a file rule needs before the next one is drawn. */
const RULE_MIN_GAP = 6;

/** How far a comment pin reaches in from the ruler's right edge. */
const PIN_WIDTH = 5;
const PIN_HEIGHT = 3;

/** Height a file's section needs before it is badged with its type icon. */
const ICON_MIN_SECTION = 24;

/**
 * Per device pixel: line widths and indents summed against how much of the
 * pixel each line covers, so dividing one by the other gives the average. Held
 * as sums rather than averages because they are accumulated a line at a time.
 */
type Lane = {
	widths: Float32Array;
	indents: Float32Array;
	/** Everything that landed here, blank lines included. */
	coverage: Float32Array;
	/** Only the lines with something on them, which is what the sums average by. */
	content: Float32Array;
};

/** A file section with room for its type icon, and where that icon goes. */
export type MinimapBadge = {
	index: number;
	top: number;
};

/**
 * Resolve every changed line down to one value per device pixel, each line
 * weighted by how much of the pixel it covers — the same box filter an image
 * uses when it is scaled down. Averaging rather than taking the widest keeps a
 * lone long line from squaring off a whole run. Nothing is drawn twice, so a
 * dense diff can't paint itself taller than the ruler.
 */
const resolveLanes = ({
	files,
	geometry,
	rows,
	scale,
	offset,
}: {
	files: Array<MinimapFile>;
	geometry: MinimapGeometry;
	rows: number;
	scale: number;
	offset: number;
}): Record<MinimapSide, Lane> => {
	const lane = (): Lane => ({
		widths: new Float32Array(rows),
		indents: new Float32Array(rows),
		coverage: new Float32Array(rows),
		content: new Float32Array(rows),
	});
	const lanes = { context: lane(), deletions: lane(), additions: lane() };

	for (const [index, file] of files.entries()) {
		const block = geometry.blocks[index];
		if (!block) continue;

		// Straight off the file's own marks: a scaled copy of every one of them,
		// on every paint, is a lot of garbage for two multiplications.
		const correction = getMinimapScale(file, block) * scale;
		const origin = block.top * scale - offset;

		// A wound-on map leaves most of the diff off either end of the ruler, and
		// the whole point of drawing at a fixed scale is that there can be a great
		// deal of it. Whole files first, then runs within the file that survives.
		if (origin > rows || origin + block.height * scale < 0) continue;

		for (const mark of file.marks) {
			const lane = lanes[mark.side];
			const markTop = origin + mark.top * correction;
			if (markTop > rows) break;
			if (markTop + mark.height * correction < 0) continue;
			// One rendered row, which is also each line's height unless wrapping gave
			// some of them more than one.
			const rowHeight = (mark.height * correction) / mark.rowCount;
			let y = markTop;

			for (const [line, width] of mark.widths.entries()) {
				const lineHeight = (mark.rows?.[line] ?? 1) * rowHeight;

				// A line owns the pixels it covers, and one thinner than a pixel owns
				// only the one its middle lands in — otherwise a removal and the
				// addition under it would both claim the boundary between them.
				const tall = lineHeight >= 1;
				const first = Math.max(Math.floor(tall ? y : y + lineHeight / 2), 0);
				const last = Math.min(tall ? Math.ceil(y + lineHeight) - 1 : first, rows - 1);

				const indent = mark.indents[line] ?? 0;
				// A line with nothing but whitespace on it marks its pixel, but averaging
				// its zero length in would drag the shape of the lines around it down.
				const blank = width === indent;

				for (let row = first; row <= last; row++) {
					// A sub-pixel line was pinned to one row, so all of it counts there.
					const covered = tall ? Math.min(y + lineHeight, row + 1) - Math.max(y, row) : lineHeight;

					lane.coverage[row] = (lane.coverage[row] ?? 0) + covered;
					if (blank) continue;

					lane.content[row] = (lane.content[row] ?? 0) + covered;
					lane.widths[row] = (lane.widths[row] ?? 0) + width * covered;
					lane.indents[row] = (lane.indents[row] ?? 0) + indent * covered;
				}
				y += lineHeight;
			}
		}
	}

	return lanes;
};

/**
 * One rule per file boundary that falls on the ruler, snapped to a whole pixel.
 * Where two would collide the taller section keeps the slot, so a sliver can't
 * take it from the file after it.
 *
 * A boundary off either end draws nothing, which is also why the first file
 * needs no special case: at rest its top sits on the ruler's own top edge,
 * where the toolbar above already draws a border.
 */
const resolveRules = ({
	geometry,
	scale,
	offset,
	limit,
}: {
	geometry: MinimapGeometry;
	scale: number;
	offset: number;
	limit: number;
}): Array<{ index: number; y: number; height: number }> => {
	const rules: Array<{ index: number; y: number; height: number }> = [];

	for (const [index, block] of geometry.blocks.entries()) {
		const y = Math.round(block.top * scale - offset);
		if (y <= 0) continue;
		if (y > limit) break;

		const rule = { index, y, height: block.height * scale };
		const previous = rules.at(-1);

		if (previous && rule.y - previous.y < RULE_MIN_GAP) {
			if (rule.height > previous.height) rules[rules.length - 1] = rule;
			continue;
		}
		rules.push(rule);
	}

	return rules;
};

/**
 * The file the top of the ruler falls inside, so it can be badged there the way
 * the files below are badged against their rules. Only worth an icon if enough
 * of it is left on the ruler to hang one on.
 */
const resolveOpening = ({
	geometry,
	scale,
	offset,
}: {
	geometry: MinimapGeometry;
	scale: number;
	offset: number;
}): Array<MinimapBadge> => {
	const index = geometry.blocks.findLastIndex((block) => block.top * scale - offset <= 0);
	const block = geometry.blocks[index];
	if (!block) return [];

	const remaining = (block.top + block.height) * scale - offset;
	return remaining >= ICON_MIN_SECTION ? [{ index, top: 0 }] : [];
};

/**
 * Paint the ruler, and report which files have room for a type icon. Badges are
 * only offered for files that kept a rule, so one always sits the same distance
 * under the line opening its section rather than measuring to one further up.
 */
export const paintMinimap = (
	canvas: HTMLCanvasElement,
	{
		files,
		geometry,
		layout,
		diffStyle,
		overlays,
	}: {
		files: Array<MinimapFile>;
		geometry: MinimapGeometry;
		layout: MinimapLayout;
		diffStyle: GUISettings["diffStyle"];
		overlays: MinimapOverlays;
	},
): Array<MinimapBadge> => {
	const { width, height } = canvas.getBoundingClientRect();
	const context = canvas.getContext("2d");
	if (!context || width === 0 || height === 0) return [];

	const ratio = globalThis.devicePixelRatio > 0 ? globalThis.devicePixelRatio : 1;
	const deviceWidth = Math.round(width * ratio);
	const deviceHeight = Math.round(height * ratio);
	if (canvas.width !== deviceWidth || canvas.height !== deviceHeight) {
		canvas.width = deviceWidth;
		canvas.height = deviceHeight;
	}

	context.setTransform(ratio, 0, 0, ratio, 0, 0);
	context.clearRect(0, 0, width, height);

	const palette = getComputedStyle(canvas);
	const fills = {
		context: palette.getPropertyValue("--minimap-context"),
		deletions: palette.getPropertyValue("--minimap-deletions"),
		additions: palette.getPropertyValue("--minimap-additions"),
		pin: palette.getPropertyValue("--minimap-pin"),
		selection: palette.getPropertyValue("--minimap-selection"),
	};

	const split = diffStyle === "split";
	const laneWidth = Math.max(split ? (width - LANE_SPLIT) / 2 : width, 1);
	// One device pixel, in the CSS units the context is scaled to.
	const thinnest = 1 / ratio;
	const { scale, offset } = layout;
	const rows = deviceHeight;
	const lanes = resolveLanes({
		files,
		geometry,
		rows,
		scale: scale * ratio,
		offset: offset * ratio,
	});

	for (let row = 0; row < rows; row++) {
		const removed = lanes.deletions.coverage[row] ?? 0;
		const added = lanes.additions.coverage[row] ?? 0;

		// A row carrying a change is described by that change, not by whatever
		// unchanged code shares the pixel with it.
		if (removed > 0 || added > 0) lanes.context.coverage[row] = 0;

		// Unified stacks both sides in one lane, so past a line per pixel a removal
		// and an addition can land on the same one. Give it to whichever fills more
		// of it, rather than letting the second side painted cover part of the first
		// and read as a line that changes colour. Ties go to removals, which come
		// first within a change.
		if (split || removed === 0 || added === 0) continue;
		const losing = added > removed ? lanes.deletions : lanes.additions;
		losing.coverage[row] = 0;
	}

	// Leading whitespace is left unpainted, so indentation reads as the gap
	// before a line's code. Every line grows from its own lane's left edge, the
	// way the text does in the column it stands for.
	const drawLane = (lane: Lane, origin: number, fill: string): void => {
		context.fillStyle = fill;

		for (let row = 0; row < rows; row++) {
			if ((lane.coverage[row] ?? 0) === 0) continue;

			// Nothing but blank lines here: the pixel still gets the single-pixel
			// minimum below, so the change stays countable rather than vanishing.
			const content = lane.content[row] ?? 0;
			const width = content === 0 ? 0 : (lane.widths[row] ?? 0) / content;
			const indent = content === 0 ? 0 : (lane.indents[row] ?? 0) / content;

			const outer = Math.max((laneWidth * width) / 255, thinnest);
			const inner = Math.min((laneWidth * indent) / 255, outer - thinnest);

			context.fillRect(origin + inner, row / ratio, outer - inner, thinnest);
		}
	};

	// Laid down before the marks so a selected hunk still reads as added or
	// removed, with the wash showing through the gaps between its lines.
	if (overlays.band) {
		context.fillStyle = fills.selection;
		context.fillRect(
			0,
			overlays.band.top * scale - offset,
			width,
			Math.max(overlays.band.height * scale, thinnest),
		);
	}

	const right = laneWidth + LANE_SPLIT;
	// Unchanged code is the same on both sides, so split shows it in both columns.
	drawLane(lanes.context, 0, fills.context);
	if (split) drawLane(lanes.context, right, fills.context);
	drawLane(lanes.deletions, 0, fills.deletions);
	drawLane(lanes.additions, split ? right : 0, fills.additions);

	// Drawn last and opaque, so a rule reads over the marks it crosses rather
	// than tinting with them.
	const rules = resolveRules({ geometry, scale, offset, limit: height - thinnest });
	context.fillStyle = palette.getPropertyValue("--minimap-file-rule");
	for (const rule of rules) context.fillRect(0, rule.y, width, thinnest);

	// Pinned to the right edge, opposite the file badges, so a commented line is
	// findable without covering the marks it belongs to. Nudged inside the ends
	// rather than clamped to them, so one just off the wound-on map stays off it.
	context.fillStyle = fills.pin;
	for (const pin of overlays.pins) {
		const top = pin * scale - offset - PIN_HEIGHT / 2;
		if (top < -PIN_HEIGHT || top > height) continue;

		context.fillRect(
			width - PIN_WIDTH,
			Math.min(Math.max(top, 0), height - PIN_HEIGHT),
			PIN_WIDTH,
			PIN_HEIGHT,
		);
	}

	return [
		...resolveOpening({ geometry, scale, offset }),
		...rules
			.filter((rule) => rule.height >= ICON_MIN_SECTION)
			.map(({ index, y }) => ({ index, top: y })),
	];
};
