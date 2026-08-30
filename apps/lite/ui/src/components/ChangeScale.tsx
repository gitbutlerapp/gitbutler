import { classes } from "#ui/components/classes.ts";
import type { ComponentProps, FC } from "react";
import styles from "./ChangeScale.module.css";

/** How many squares the bar is made of. */
const SEGMENTS = 5;

/**
 * Splits the squares between the two sides in proportion to the lines each one
 * touched. A side that changed anything at all keeps a square, so `+1 -900`
 * still reads as a change with an addition in it rather than a pure deletion.
 */
const splitSegments = (added: number, removed: number): { added: number; removed: number } => {
	let addedSegments = Math.round((added / (added + removed)) * SEGMENTS);
	if (added > 0 && addedSegments === 0) addedSegments = 1;
	if (removed > 0 && addedSegments === SEGMENTS) addedSegments = SEGMENTS - 1;
	return { added: addedSegments, removed: SEGMENTS - addedSegments };
};

type Props = {
	added: number;
	removed: number;
} & ComponentProps<"span">;

/**
 * The green/red squares beside a diff's `+N -N` counts, showing at a glance how
 * much of a change is additions and how much is deletions. Decorative: the
 * counts next to it carry the numbers, so it is hidden from screen readers.
 *
 * Renders nothing when nothing changed.
 */
export const ChangeScale: FC<Props> = ({ added, removed, ...props }) => {
	if (added === 0 && removed === 0) return null;

	const segments = splitSegments(added, removed);

	return (
		<span {...props} aria-hidden className={classes(props.className, styles.container)}>
			{Array.from({ length: SEGMENTS }, (_, index) => (
				<span key={index} className={index < segments.added ? styles.added : styles.removed} />
			))}
		</span>
	);
};
