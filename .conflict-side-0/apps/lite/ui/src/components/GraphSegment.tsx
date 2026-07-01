import styles from "./GraphSegment.module.css";
import { classes } from "#ui/components/classes.ts";
import { ComponentProps, FC } from "react";
import { CommitState } from "@gitbutler/but-sdk";

const glyphPaths = {
	parent: "M8 0V28",
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

const groupGlyph = (
	<>
		<path opacity="0.4" d="M8 0V7M8 21L8 28" stroke="currentColor" strokeWidth="1.5" />
		<path
			d="M11.0862 12.1524C11.3502 11.6602 11.5 11.0976 11.5 10.5C11.5 8.567 9.933 7 8 7C6.067 7 4.5 8.567 4.5 10.5C4.5 11.0976 4.64977 11.6602 4.91382 12.1524M5 15.8038C4.68259 15.277 4.5 14.6598 4.5 14C4.5 12.067 6.067 10.5 8 10.5C9.933 10.5 11.5 12.067 11.5 14C11.5 14.6598 11.3174 15.277 11 15.8038M11.5 17.5C11.5 19.433 9.933 21 8 21C6.067 21 4.5 19.433 4.5 17.5C4.5 15.567 6.067 14 8 14C9.933 14 11.5 15.567 11.5 17.5Z"
			stroke="currentColor"
			strokeWidth="1.5"
		/>
	</>
);

/** @public */
export type GraphSegmentGlyph = keyof typeof glyphPaths | "commit" | "group";

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

export type GraphSegmentStatus = "Diverged" | CommitState["type"];

interface GraphSegmentProps extends ComponentProps<"div"> {
	glyph: GraphSegmentGlyph;
	status: GraphSegmentStatus;
}

export const GraphSegment: FC<GraphSegmentProps> = ({ glyph, className, status, ...props }) => (
	<div {...props} className={classes(className, styles.container)} data-status={status}>
		<svg
			className={styles.mainSegment}
			viewBox="0 0 16 28"
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
			aria-hidden="true"
			focusable="false"
		>
			{glyph === "commit" ? (
				commitGlyph
			) : glyph === "group" ? (
				groupGlyph
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
