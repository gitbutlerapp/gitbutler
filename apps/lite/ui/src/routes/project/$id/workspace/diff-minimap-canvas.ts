import type { GUISettings } from "#electron/settings.ts";
import { getMinimapScale, type MinimapFile, type MinimapGeometry } from "./diff-minimap.ts";

/** The channel between the two lanes in side-by-side. */
const LANE_SPLIT = 1;

/** Vertical room a file rule needs before the next one is drawn. */
const RULE_MIN_GAP = 6;

/** Height a file's section needs before it is badged with its type icon. */
const ICON_MIN_SECTION = 24;

type Side = "additions" | "deletions";

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
}: {
	files: Array<MinimapFile>;
	geometry: MinimapGeometry;
	rows: number;
	scale: number;
}): Record<Side, Lane> => {
	const lane = (): Lane => ({
		widths: new Float32Array(rows),
		indents: new Float32Array(rows),
		coverage: new Float32Array(rows),
		content: new Float32Array(rows),
	});
	const lanes = { deletions: lane(), additions: lane() };

	for (const [index, file] of files.entries()) {
		const block = geometry.blocks[index];
		if (!block) continue;

		// Straight off the file's own marks: a scaled copy of every one of them,
		// on every paint, is a lot of garbage for two multiplications.
		const correction = getMinimapScale(file, block) * scale;
		const origin = block.top * scale;

		for (const mark of file.marks) {
			const lane = lanes[mark.side];
			const lineHeight = (mark.height * correction) / mark.widths.length;
			let y = origin + mark.top * correction;

			for (const [line, width] of mark.widths.entries()) {
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
 * One rule per file, snapped to a whole pixel. Where two would collide the
 * taller section keeps the slot, so a sliver can't take it from the file after
 * it. The first file gets none — the ruler's top edge is already where it
 * starts, and the toolbar above draws its own border there.
 */
const resolveRules = ({
	geometry,
	scale,
	limit,
}: {
	geometry: MinimapGeometry;
	scale: number;
	limit: number;
}): Array<{ index: number; y: number; height: number }> => {
	const rules: Array<{ index: number; y: number; height: number }> = [];

	for (const [index, block] of geometry.blocks.entries()) {
		if (index === 0) continue;

		const rule = {
			index,
			y: Math.min(Math.round(block.top * scale), limit),
			height: block.height * scale,
		};
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
 * Paint the ruler, and report which files have room for a type icon. Badges are
 * only offered for files that kept a rule, so one always sits the same distance
 * under the line opening its section rather than measuring to one further up.
 */
export const paintMinimap = (
	canvas: HTMLCanvasElement,
	{
		files,
		geometry,
		diffStyle,
	}: {
		files: Array<MinimapFile>;
		geometry: MinimapGeometry;
		diffStyle: GUISettings["diffStyle"];
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
		deletions: palette.getPropertyValue("--minimap-deletions"),
		additions: palette.getPropertyValue("--minimap-additions"),
	};

	const split = diffStyle === "split";
	const laneWidth = Math.max(split ? (width - LANE_SPLIT) / 2 : width, 1);
	// One device pixel, in the CSS units the context is scaled to.
	const thinnest = 1 / ratio;
	const scale = height / geometry.contentHeight;
	const rows = deviceHeight;
	const lanes = resolveLanes({ files, geometry, rows, scale: scale * ratio });

	// Unified stacks both sides in one lane, so past a line per pixel a removal
	// and an addition can land on the same one. Give it to whichever fills more
	// of it, rather than letting the second side painted cover part of the first
	// and read as a line that changes colour.
	if (!split) {
		for (let row = 0; row < rows; row++) {
			const removed = lanes.deletions.coverage[row] ?? 0;
			const added = lanes.additions.coverage[row] ?? 0;
			if (removed === 0 || added === 0) continue;

			// Ties go to removals, which come first within a change.
			const losing = added > removed ? lanes.deletions : lanes.additions;
			losing.coverage[row] = 0;
		}
	}

	// Leading whitespace is left unpainted, so indentation reads as the gap
	// before a line's code. Every line grows from its own lane's left edge, the
	// way the text does in the column it stands for.
	for (const [side, lane] of Object.entries(lanes) as Array<[Side, Lane]>) {
		const origin = split && side === "additions" ? laneWidth + LANE_SPLIT : 0;
		context.fillStyle = fills[side];

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
	}

	// Drawn last and opaque, so a rule reads over the marks it crosses rather
	// than tinting with them.
	const rules = resolveRules({ geometry, scale, limit: height - thinnest });
	context.fillStyle = palette.getPropertyValue("--minimap-file-rule");
	for (const rule of rules) context.fillRect(0, rule.y, width, thinnest);

	const first = geometry.blocks[0];
	const opening = first && first.height * scale >= ICON_MIN_SECTION ? [{ index: 0, top: 0 }] : [];

	return [
		...opening,
		...rules
			.filter((rule) => rule.height >= ICON_MIN_SECTION)
			.map(({ index, y }) => ({ index, top: y })),
	];
};
