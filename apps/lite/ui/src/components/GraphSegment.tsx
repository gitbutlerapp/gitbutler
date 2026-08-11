import styles from "./GraphSegment.module.css";
import { classes } from "#ui/components/classes.ts";
import type { ComponentProps, FC } from "react";
import type { CommitState } from "@gitbutler/but-sdk";

const glyphPaths = {
	parent: "M8 0V28",
	/* The parent line with its overhead dropped, for a row that starts a rail
	   rather than continuing one. It begins where the group glyph's rings do, so
	   a fold toggle heading a rail starts its line in the same place whether the
	   run it holds is folded away or on screen. */
	parentHead: "M8 3V28",
	horizontal: "M-9.53674e-07 14L16 14",
	space: "",
	// Forks
	forkLeft: "M-5.96046e-08 14H2C5.31371 14 8 16.6863 8 20V28",
	forkRight: "M16 14H14C10.6863 14 8 16.6863 8 20V28",
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

const commitGlyph = (
	<>
		<path opacity="0.4" d="M8 0V11M8 17V28" stroke="currentColor" strokeWidth="1.5" />
		<path
			d="M11.5 14C11.5 15.933 9.933 17.5 8 17.5C6.067 17.5 4.5 15.933 4.5 14C4.5 12.067 6.067 10.5 8 10.5C9.933 10.5 11.5 12.067 11.5 14Z"
			stroke="currentColor"
			strokeWidth="1.5"
		/>
	</>
);

const groupRingsPath =
	"M11.0862 8.1524C11.3502 7.6602 11.5 7.0976 11.5 6.5C11.5 4.567 9.933 3 8 3C6.067 3 4.5 4.567 4.5 6.5C4.5 7.0976 4.64977 7.6602 4.91382 8.1524M5 11.8038C4.68259 11.277 4.5 10.6598 4.5 10C4.5 8.067 6.067 6.5 8 6.5C9.933 6.5 11.5 8.067 11.5 10C11.5 10.6598 11.3174 11.277 11 11.8038M11.5 13.5C11.5 15.433 9.933 17 8 17C6.067 17 4.5 15.433 4.5 13.5C4.5 11.567 6.067 10 8 10C9.933 10 11.5 11.567 11.5 13.5Z";

const groupGlyph = (
	<>
		<path opacity="0.4" d="M8 0V2.78571M8 17.0038V26" stroke="currentColor" strokeWidth="1.5" />
		<path d={groupRingsPath} stroke="currentColor" strokeWidth="1.5" />
	</>
);

/** The rings without the tail above them, for a row that starts a rail. */
const groupHeadGlyph = (
	<>
		<path opacity="0.4" d="M8 17.0038V26" stroke="currentColor" strokeWidth="1.5" />
		<path d={groupRingsPath} stroke="currentColor" strokeWidth="1.5" />
	</>
);

/** @public */
export type GraphSegmentGlyph = keyof typeof glyphPaths | "commit" | "group" | "groupHead";

/** Both are drawn on the group glyph's shorter canvas. */
const isGroupGlyph = (glyph: GraphSegmentGlyph): boolean =>
	glyph === "group" || glyph === "groupHead";

/**
 * Glyphs whose rail carries on past the drawing, so a taller row goes on
 * drawing it. The head glyphs are left out: what they start is the rail below
 * them, and the band would draw over the very space they exist to leave empty
 * in a column that stacks upwards.
 */
const stretchableGlyphs = new Set<GraphSegmentGlyph>([
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

interface GraphSegmentProps extends ComponentProps<"div"> {
	glyph: GraphSegmentGlyph;
	status: GraphSegmentStatus;
}

export const GraphSegment: FC<GraphSegmentProps> = ({ glyph, className, status, ...props }) => (
	<div {...props} className={classes(className, styles.container)} data-status={status}>
		<svg
			className={classes(styles.mainSegment, isGroupGlyph(glyph) && styles.groupSegment)}
			viewBox={isGroupGlyph(glyph) ? "0 0 16 26" : "0 0 16 28"}
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
			aria-hidden="true"
			focusable="false"
		>
			{glyph === "commit" ? (
				commitGlyph
			) : glyph === "group" ? (
				groupGlyph
			) : glyph === "groupHead" ? (
				groupHeadGlyph
			) : (
				<path d={glyphPaths[glyph]} opacity="0.4" stroke="currentColor" strokeWidth="1.5" />
			)}
		</svg>

		{stretchableGlyphs.has(glyph) && (
			<svg
				viewBox="0 0 16 28"
				preserveAspectRatio="none"
				fill="none"
				xmlns="http://www.w3.org/2000/svg"
				className={styles.stretchSegment}
				aria-hidden="true"
				focusable="false"
			>
				<path d={glyphPaths.parent} opacity="0.4" stroke="currentColor" strokeWidth="1.5" />
			</svg>
		)}
	</div>
);
