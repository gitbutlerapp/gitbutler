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
 * Room a badge takes below where it hangs, its own height and the margin
 * clearing the rule. One offered past the end of the ruler would still be
 * placed there, and an absolute box hanging out of the ruler makes an ancestor
 * scrollable — so a wheel over the map would scroll the pane instead.
 */
const ICON_HEIGHT = 23;

/**
 * Coverage per pixel, rather than an average width per pixel row.
 *
 * Where a row holds one line the two say the same thing. Where it holds many,
 * an average is a single number standing in for a distribution — and every
 * such number is wrong in its own way: the widest squares the run off, the
 * narrowest cuts it short, the mean lands where no line actually ends. Coverage
 * keeps all of them: the row is solid as far as every line reaches and thins
 * out towards the longest, and that fade is the shape of the code.
 *
 * Held as a difference array — coverage added where a line starts and taken
 * away where it ends — so a line costs two writes however long it is, and one
 * running sum along the row turns them back into coverage per pixel.
 */
type Lane = {
	/** Two entries per line, on a row of its own: `stride` wide, `columns` used. */
	edges: Float32Array;
	/** Everything that landed on the row, which the running sums divide by. */
	coverage: Float32Array;
};

/** Kept between paints: these run to megabytes, and a winding map repaints per frame. */
const held = new Map<string, Float32Array>();

const scratch = (key: string, length: number): Float32Array => {
	const kept = held.get(key);
	if (kept?.length === length) {
		kept.fill(0);
		return kept;
	}

	const made = new Float32Array(length);
	held.set(key, made);
	return made;
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
	columns,
	scale,
	offset,
}: {
	files: Array<MinimapFile>;
	geometry: MinimapGeometry;
	rows: number;
	columns: number;
	scale: number;
	offset: number;
}): Record<MinimapSide, Lane> => {
	// One past the last column, so a line reaching the far edge has somewhere to
	// be taken away again.
	const stride = columns + 1;
	const lane = (name: MinimapSide): Lane => ({
		edges: scratch(`${name}.edges`, rows * stride),
		coverage: scratch(`${name}.coverage`, rows),
	});
	const lanes = {
		context: lane("context"),
		deletions: lane("deletions"),
		additions: lane("additions"),
	};

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

				// Leading whitespace is left uncovered, so indentation reads as the gap
				// before a line's code. Every line marks at least the one pixel, however
				// little of the lane it fills.
				const start = Math.min(Math.round((indent * columns) / 255), columns - 1);
				const end = Math.min(Math.max(Math.round((width * columns) / 255), start + 1), columns);

				for (let row = first; row <= last; row++) {
					// A sub-pixel line was pinned to one row, so all of it counts there.
					const covered = tall ? Math.min(y + lineHeight, row + 1) - Math.max(y, row) : lineHeight;
					const base = row * stride;

					lane.coverage[row] = (lane.coverage[row] ?? 0) + covered;
					lane.edges[base + start] = (lane.edges[base + start] ?? 0) + covered;
					lane.edges[base + end] = (lane.edges[base + end] ?? 0) - covered;
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
 * Marks are composited through a surface of their own so the selection band
 * keeps showing between the lines it covers: writing pixels straight onto the
 * ruler would put them over it rather than on top of it.
 */
const surface: { canvas: HTMLCanvasElement | null; image: ImageData | null } = {
	canvas: null,
	image: null,
};

/**
 * Canvas takes a colour as CSS writes it, but pixels have to be numbers — and
 * these arrive as whatever `color-mix` resolved to. Painting one and reading it
 * back is the only reader of modern colour syntax we have to hand.
 */
const swatch: { canvas: HTMLCanvasElement | null; read: Map<string, [number, number, number]> } = {
	canvas: null,
	read: new Map(),
};

const resolveColour = (colour: string): [number, number, number] => {
	const known = swatch.read.get(colour);
	if (known) return known;

	swatch.canvas ??= document.createElement("canvas");
	swatch.canvas.width = 1;
	swatch.canvas.height = 1;

	const context = swatch.canvas.getContext("2d", { willReadFrequently: true });
	if (!context) return [0, 0, 0];

	context.clearRect(0, 0, 1, 1);
	context.fillStyle = colour;
	context.fillRect(0, 0, 1, 1);

	const [red = 0, green = 0, blue = 0] = context.getImageData(0, 0, 1, 1).data;
	const resolved: [number, number, number] = [red, green, blue];
	swatch.read.set(colour, resolved);
	return resolved;
};

/**
 * The tokens the ruler is drawn in. Reading them costs a style recalc, and the
 * ruler writes its own inline properties just before every paint — so the recalc
 * is never one the browser has already done. They only move with the colour
 * scheme, which is a thing the ruler is told about.
 */
type MinimapPalette = {
	context: [number, number, number];
	deletions: [number, number, number];
	additions: [number, number, number];
	pin: string;
	selection: string;
	rule: string;
};

let palette: MinimapPalette | null = null;

/** Drop the cached tokens, for a caller that knows they now resolve differently. */
export const forgetMinimapPalette = (): void => {
	palette = null;
};

const readMinimapPalette = (canvas: HTMLCanvasElement): MinimapPalette => {
	if (palette) return palette;

	const tokens = getComputedStyle(canvas);
	palette = {
		context: resolveColour(tokens.getPropertyValue("--minimap-context")),
		deletions: resolveColour(tokens.getPropertyValue("--minimap-deletions")),
		additions: resolveColour(tokens.getPropertyValue("--minimap-additions")),
		pin: tokens.getPropertyValue("--minimap-pin"),
		selection: tokens.getPropertyValue("--minimap-selection"),
		rule: tokens.getPropertyValue("--minimap-file-rule"),
	};
	return palette;
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

	const fills = readMinimapPalette(canvas);

	const split = diffStyle === "split";
	// One device pixel, in the CSS units the context is scaled to.
	const thinnest = 1 / ratio;
	const { scale, offset } = layout;
	const rows = deviceHeight;
	const channel = Math.round(LANE_SPLIT * ratio);
	const columns = Math.max(split ? Math.floor((deviceWidth - channel) / 2) : deviceWidth, 1);
	const stride = columns + 1;
	const lanes = resolveLanes({
		files,
		geometry,
		rows,
		columns,
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

	/**
	 * Dividing the running sum by the row's own coverage gives the share of its
	 * lines reaching each pixel: a row of one line is solid to its end, a row of
	 * many fades out across the lengths they run to.
	 */
	const paintLane = (
		data: Uint8ClampedArray,
		lane: Lane,
		origin: number,
		[red, green, blue]: [number, number, number],
	): void => {
		for (let row = 0; row < rows; row++) {
			const covered = lane.coverage[row] ?? 0;
			if (covered === 0) continue;

			const base = row * stride;
			let running = 0;

			for (let column = 0; column < columns; column++) {
				running += lane.edges[base + column] ?? 0;
				if (running <= 0) continue;

				const alpha = Math.min(running / covered, 1);
				const at = (row * deviceWidth + origin + column) * 4;

				// Over whatever a lane before it left, so the two columns of a split
				// diff and the context under them compose rather than overwrite.
				const beneath = ((data[at + 3] ?? 0) / 255) * (1 - alpha);
				const total = alpha + beneath;
				if (total <= 0) continue;

				data[at] = (red * alpha + (data[at] ?? 0) * beneath) / total;
				data[at + 1] = (green * alpha + (data[at + 1] ?? 0) * beneath) / total;
				data[at + 2] = (blue * alpha + (data[at + 2] ?? 0) * beneath) / total;
				data[at + 3] = total * 255;
			}
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

	surface.canvas ??= document.createElement("canvas");
	if (surface.canvas.width !== deviceWidth || surface.canvas.height !== deviceHeight) {
		surface.canvas.width = deviceWidth;
		surface.canvas.height = deviceHeight;
		surface.image = null;
	}

	const marks = surface.canvas.getContext("2d");
	if (marks) {
		surface.image ??= marks.createImageData(deviceWidth, deviceHeight);
		surface.image.data.fill(0);

		const right = columns + channel;
		// Unchanged code is the same on both sides, so split shows it in both columns.
		paintLane(surface.image.data, lanes.context, 0, fills.context);
		if (split) paintLane(surface.image.data, lanes.context, right, fills.context);
		paintLane(surface.image.data, lanes.deletions, 0, fills.deletions);
		paintLane(surface.image.data, lanes.additions, split ? right : 0, fills.additions);

		marks.putImageData(surface.image, 0, 0);
		// Drawn rather than written, so the band beneath shows through the gaps.
		context.drawImage(surface.canvas, 0, 0, width, height);
	}

	// Drawn last and opaque, so a rule reads over the marks it crosses rather
	// than tinting with them.
	const rules = resolveRules({ geometry, scale, offset, limit: height - thinnest });
	context.fillStyle = fills.rule;
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
			.filter((rule) => rule.height >= ICON_MIN_SECTION && rule.y + ICON_HEIGHT <= height)
			.map(({ index, y }) => ({ index, top: y })),
	];
};
