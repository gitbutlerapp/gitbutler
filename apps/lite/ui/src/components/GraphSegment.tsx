import styles from "./GraphSegment.module.css";
import { classes } from "#ui/components/classes.ts";
import { GRAPH_COMMIT_BEND_PADDING, GRAPH_LANE_WIDTH } from "./graph-spacing.ts";
import type { ComponentProps, FC } from "react";
import type { CommitState } from "@gitbutler/but-sdk";

const ARC_K = 0.5523;
const n = (value: number): string => String(Math.round(value * 100) / 100);
const trunkX = 8 - GRAPH_LANE_WIDTH;
const hookRadius = Math.min(4, GRAPH_LANE_WIDTH / 2);
const hookK = ARC_K * hookRadius;
const hookHeadPath = `M${trunkX} 0V${14 - hookRadius}C${trunkX} ${n(14 - hookRadius + hookK)} ${n(trunkX + hookRadius - hookK)} 14 ${trunkX + hookRadius} 14`;
const laneStyle = { width: GRAPH_LANE_WIDTH };
const gapInsetStyle = { marginInlineStart: -GRAPH_LANE_WIDTH };
const trunkStyle = { ...laneStyle, ...gapInsetStyle };
const commitBendStyle = { height: 28 + GRAPH_COMMIT_BEND_PADDING };

const glyphPaths = {
	parent: "M8 0V28",
	horizontal: "M-9.53674e-07 14L16 14",
	space: "",
	// Forks
	forkLeft: "M-5.96046e-08 14H2C5.31371 14 8 16.6863 8 20V28",
	forkRight: "M16 14H14C10.6863 14 8 16.6863 8 20V28",
	/** A tick off the trunk near the panel's edge. */
	notch: `M${trunkX} 14H${trunkX + 5}`,
	/** The trunk joins the history's column through two quarter turns. */
	hook: `${hookHeadPath}H${8 - hookRadius}C${n(8 - hookRadius + hookK)} 14 8 ${n(14 + hookRadius - hookK)} 8 ${14 + hookRadius}V28`,
	forkBoth: "M0 14H8M16 14H8M8 28L8 14",
	// Merges
	mergeLeft: "M-5.96046e-08 14H2C5.31371 14 8 11.3137 8 8V2.38419e-07",
	mergeRight: "M16 14H14C10.6863 14 8 11.3137 8 8V2.38419e-07",
	mergeBoth: "M0 14H8M16 14H8M8 14L8 0",
	// Joins
	joinLeft: "M8 14H0M8 14V0M8 14V28",
	joinRight: "M16 14H8M8 14V0M8 14V28",
	joinBoth: "M16 14L8 14M0 14H8M8 0V14M8 28V14",
};

/** A stretch of the rail in another status's colour, or the glyph's own when none is given. */
const Tone: FC<{ status: GraphSegmentStatus | undefined; d: string }> = ({ status, d }) => (
	<g className={styles.tone} data-status={status}>
		<path className={styles.line} d={d} strokeWidth="1.5" />
	</g>
);

/**
 * The columns the main line runs through behind a row or gap. The first is
 * the trunk's on the panel's edge, drawn the way the gaps draw it so the two
 * land on the same pixels: an SVG antialiases where a CSS box snaps. Folded
 * below the row, the trunk's tail fades out, a hint of what lies below.
 */
const passes = (behind: number, folded = false) =>
	Array.from({ length: behind }, (_, column) =>
		column === 0 ? (
			<svg
				key={column}
				className={classes(styles.edgePass, folded && styles.edgePassFading)}
				style={trunkStyle}
				viewBox={`0 0 ${GRAPH_LANE_WIDTH} 28`}
				preserveAspectRatio="none"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
				aria-hidden="true"
				focusable="false"
			>
				<path d="M8 0V28" strokeWidth="1.5" />
			</svg>
		) : (
			<span key={column} className={styles.pass} style={laneStyle} aria-hidden />
		),
	);

const ringPath =
	"M11.5 14C11.5 15.933 9.933 17.5 8 17.5C6.067 17.5 4.5 15.933 4.5 14C4.5 12.067 6.067 10.5 8 10.5C9.933 10.5 11.5 12.067 11.5 14Z";

const ring = <path d={ringPath} stroke="currentColor" strokeWidth="1.5" />;

const commitGlyph = (
	above: GraphSegmentStatus | undefined,
	below: GraphSegmentStatus | undefined,
	railEnds: boolean,
	fromTrunk: boolean,
) => (
	<>
		{fromTrunk ? (
			<g transform={`translate(16 ${-GRAPH_COMMIT_BEND_PADDING}) scale(-1 1)`}>
				<Tone status={above} d={bendPath(11 + GRAPH_COMMIT_BEND_PADDING)} />
			</g>
		) : (
			<Tone status={above} d="M8 0V11" />
		)}
		{ring}
		{!railEnds && <Tone status={below} d="M8 17V28" />}
	</>
);

const groupRingsPath =
	"M11.0862 8.1524C11.3502 7.6602 11.5 7.0976 11.5 6.5C11.5 4.567 9.933 3 8 3C6.067 3 4.5 4.567 4.5 6.5C4.5 7.0976 4.64977 7.6602 4.91382 8.1524M5 11.8038C4.68259 11.277 4.5 10.6598 4.5 10C4.5 8.067 6.067 6.5 8 6.5C9.933 6.5 11.5 8.067 11.5 10C11.5 10.6598 11.3174 11.277 11 11.8038M11.5 13.5C11.5 15.433 9.933 17 8 17C6.067 17 4.5 15.433 4.5 13.5C4.5 11.567 6.067 10 8 10C9.933 10 11.5 11.567 11.5 13.5Z";

const groupGlyph = (
	<>
		<path className={styles.line} d="M8 0V2.78571M8 17.0038V26" strokeWidth="1.5" />
		<path d={groupRingsPath} stroke="currentColor" strokeWidth="1.5" />
	</>
);

/** A branch's tick on a rail that continues above: the stretch above and below it in others' colours. */
const joinRightGlyph = (
	above: GraphSegmentStatus | undefined,
	below: GraphSegmentStatus | undefined,
) => (
	<>
		<Tone status={above} d="M8 14V0" />
		<path className={styles.line} d="M16 14H8" strokeWidth="1.5" />
		<Tone status={below} d="M8 14V28" />
	</>
);

/** A plain rail through the row, the stretch below its centre in another's colour. */
const parentGlyph = (below: GraphSegmentStatus | undefined) => (
	<>
		<path className={styles.line} d="M8 0V14" strokeWidth="1.5" />
		<Tone status={below} d="M8 14V28" />
	</>
);

/** A branch's tick starting a rail: the stretch below it in another's colour. */
const forkRightGlyph = (below: GraphSegmentStatus | undefined) => (
	<>
		<path className={styles.line} d="M16 14H14C10.6863 14 8 16.6863 8 20" strokeWidth="1.5" />
		<Tone status={below} d="M8 20V28" />
	</>
);

/** The rings on the row's centre line, for a single-line row of their own. */
const groupCenteredGlyph = (
	<>
		<path className={styles.line} d="M8 0V6.7857M8 21.0038V28" strokeWidth="1.5" />
		<g transform="translate(0 4)">
			<path d={groupRingsPath} stroke="currentColor" strokeWidth="1.5" />
		</g>
	</>
);

/** The centred rings without the tail below them, for the row a rail ends on. */
const groupCenteredFootGlyph = (
	<>
		<path className={styles.line} d="M8 0V6.7857" strokeWidth="1.5" />
		<g transform="translate(0 4)">
			<path d={groupRingsPath} stroke="currentColor" strokeWidth="1.5" />
		</g>
	</>
);

/** @public */
export type GraphSegmentGlyph = keyof typeof glyphPaths | "commit" | "group";

/** Glyphs whose rail carries on past the drawing, so a taller row goes on drawing it. */
const stretchableGlyphs = new Set<GraphSegmentGlyph>([
	"hook",
	"parent",
	"commit",
	"group",
	"forkLeft",
	"forkRight",
	"forkBoth",
	"joinLeft",
	"joinRight",
	"joinBoth",
]);

/**
 * `Upstream` has no counterpart in {@link CommitState}: it describes the target
 * branch's own line — commits that are on the target and not in the workspace
 * at all, rather than commits of ours in some state against it.
 */
export type GraphSegmentStatus = "Diverged" | "Upstream" | CommitState["type"];

interface GraphSegmentProps extends ComponentProps<"span"> {
	glyph: GraphSegmentGlyph;
	status: GraphSegmentStatus;
	/** The rail ends on this row: no tail below the icon, nothing stretched under a taller row. */
	railEnds?: boolean;
	/** What lies below the row is folded away: the trunk's tail behind the row fades out, a hint of it. */
	folded?: boolean;
	/** The rings sit on the row's centre line, for a single-line row of their own. */
	centered?: boolean;
	/** The rail above or below the icon in another status's colour; a stretch between two icons is the lower one's. */
	above?: GraphSegmentStatus;
	below?: GraphSegmentStatus;
	/** Bend from the trunk one column left into this commit's ring. */
	fromTrunk?: boolean;
	/** How many columns of the main line run behind the row, left of the glyph. */
	behind?: number;
}

export const GraphSegment: FC<GraphSegmentProps> = ({
	glyph,
	className,
	status,
	railEnds = false,
	folded = false,
	centered = false,
	above,
	below,
	fromTrunk = false,
	behind = 0,
	...props
}) => (
	// Spans throughout: the segment sits in buttons and spans, which take phrasing content only.
	<span {...props} className={classes(className, styles.container)} data-status={status}>
		{passes(behind, folded)}
		<span className={styles.glyph}>
			<svg
				className={classes(
					styles.mainSegment,
					glyph === "group" && !centered && styles.groupSegment,
				)}
				viewBox={
					fromTrunk
						? `0 ${-GRAPH_COMMIT_BEND_PADDING} 16 ${28 + GRAPH_COMMIT_BEND_PADDING}`
						: glyph === "group" && !centered
							? "0 0 16 26"
							: "0 0 16 28"
				}
				style={fromTrunk ? commitBendStyle : undefined}
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
				aria-hidden="true"
				focusable="false"
			>
				{glyph === "commit" ? (
					commitGlyph(above, below, railEnds, fromTrunk)
				) : railEnds && glyph === "group" && centered ? (
					groupCenteredFootGlyph
				) : railEnds && glyph === "hook" ? (
					<path className={styles.line} d={`${hookHeadPath}H8`} strokeWidth="1.5" />
				) : glyph === "joinRight" ? (
					joinRightGlyph(above, below)
				) : glyph === "parent" ? (
					parentGlyph(below)
				) : glyph === "forkRight" ? (
					forkRightGlyph(below)
				) : glyph === "group" ? (
					centered ? (
						groupCenteredGlyph
					) : (
						groupGlyph
					)
				) : (
					<path className={styles.line} d={glyphPaths[glyph]} strokeWidth="1.5" />
				)}
			</svg>

			{stretchableGlyphs.has(glyph) && !railEnds && (
				<svg
					viewBox="0 0 16 28"
					preserveAspectRatio="none"
					fill="none"
					xmlns="http://www.w3.org/2000/svg"
					className={styles.stretchSegment}
					aria-hidden="true"
					focusable="false"
				>
					<Tone status={below} d={glyphPaths.parent} />
				</svg>
			)}
		</span>
	</span>
);

/**
 * The bend from the column right of the main line onto it, through a gap of
 * this height: two quarter turns joined by a straight, meeting the
 * line at the gap's foot.
 */
const bendPath = (height: number): string => {
	const r = Math.min(4, GRAPH_LANE_WIDTH / 2, height / 2);
	const k = ARC_K * r;
	const my = height / 2;
	const from = 8 + GRAPH_LANE_WIDTH;
	return [
		`M${from} 0 V${n(my - r)}`,
		`C${from} ${n(my - r + k)} ${n(from - (r - k))} ${n(my)} ${from - r} ${n(my)}`,
		`L${8 + r} ${n(my)}`,
		`C${n(8 + (r - k))} ${n(my)} 8 ${n(my + r - k)} 8 ${n(my + r)}`,
		`V${n(height)}`,
	].join(" ");
};

const edgePaths = {
	parent: "M1 0V28",
	/** The trunk's head: the row's tick bending down onto the line. */
	forkRight: "M8 14C4.134 14 1 17.134 1 21V28",
};

/**
 * The trunk's own column at the panel's edge, for the rows it runs down as
 * their rail: 8px wide, its line hugging the border, with room for a straight
 * run or the bend at its head and no glyph. Rows on it carry no inset.
 */
export const GraphEdge: FC<{ glyph: keyof typeof edgePaths }> = ({ glyph }) => (
	<span className={styles.container} data-status="LocalOnly">
		<span className={classes(styles.glyph, styles.edge)}>
			<svg
				className={styles.mainSegment}
				viewBox="0 0 8 28"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
				aria-hidden="true"
				focusable="false"
			>
				<path className={styles.line} d={edgePaths[glyph]} strokeWidth="1.5" />
			</svg>
			<svg
				viewBox="0 0 8 28"
				preserveAspectRatio="none"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
				className={styles.stretchSegment}
				aria-hidden="true"
				focusable="false"
			>
				<path className={styles.line} d={edgePaths.parent} strokeWidth="1.5" />
			</svg>
		</span>
	</span>
);

/**
 * The gutter of the gap under a card: the main line runs through it, and the
 * card's own line, a column to the right, may bend onto it. With columns
 * `behind`, the gap sits that far right, the lines behind it passing through;
 * with none, the main line is the trunk on the panel's edge.
 */
export const GraphGap: FC<{
	height: number;
	bend?: GraphSegmentStatus;
	behind?: number;
}> = ({ height, bend, behind = 0 }) => (
	<div className={styles.gap} style={{ height }} aria-hidden>
		{passes(behind)}
		<svg
			viewBox={`0 0 28 ${height}`}
			style={behind === 0 ? gapInsetStyle : undefined}
			width="28"
			height={height}
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
			aria-hidden="true"
			focusable="false"
		>
			<Tone status="LocalOnly" d={`M8 0V${height}`} />
			{bend !== undefined && <Tone status={bend} d={bendPath(height)} />}
		</svg>
	</div>
);
