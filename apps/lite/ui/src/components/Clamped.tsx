import { classes } from "#ui/components/classes.ts";
import type { CSSProperties, FC, ReactNode } from "react";
import { useLayoutEffect, useRef, useState } from "react";
import styles from "./Clamped.module.css";

/** Resolve the supported clamp lengths ("240px", "80vh") to pixels. */
const resolveLength = (length: string): number => {
	const match = /^(\d+(?:\.\d+)?)(px|vh)$/.exec(length);
	if (match === null) return Number.POSITIVE_INFINITY;
	const value = Number(match[1]);
	return match[2] === "vh" ? (value * window.innerHeight) / 100 : value;
};

/**
 * Caps content at `maxHeight` with a fade and a Show more/less toggle,
 * folding only when content is actually taller than the cap. Content that
 * changes size after mount (lazy images, async syntax highlighting)
 * re-measures via a ResizeObserver on the inner wrapper, and viewport
 * resizes re-measure the vh-based caps.
 */
export const Clamped: FC<{
	/** A px or vh length, e.g. `"240px"` or `"80vh"`. */
	maxHeight: string;
	/** Don't fold when the full content already fits within the viewport. */
	skipWhenViewportFits?: boolean;
	children: ReactNode;
}> = ({ maxHeight, skipWhenViewportFits = false, children }) => {
	const [expanded, setExpanded] = useState(false);
	const [folded, setFolded] = useState(false);
	const innerRef = useRef<HTMLDivElement | null>(null);

	useLayoutEffect(() => {
		if (expanded) return;
		const inner = innerRef.current;
		if (inner === null) return;

		// The inner wrapper always has its natural height (only the outer box
		// is clamped), so this needs no unfold-and-measure dance.
		const measure = () => {
			const contentHeight = inner.offsetHeight;
			const fitsViewport = skipWhenViewportFits && contentHeight <= window.innerHeight;
			setFolded(contentHeight > resolveLength(maxHeight) + 1 && !fitsViewport);
		};
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(inner);
		window.addEventListener("resize", measure);
		return () => {
			observer.disconnect();
			window.removeEventListener("resize", measure);
		};
	}, [expanded, maxHeight, skipWhenViewportFits]);

	const isFolded = !expanded && folded;

	return (
		<>
			<div
				style={{ "--clamp-max-height": maxHeight } as CSSProperties}
				className={classes(isFolded && styles.clamped, isFolded && styles.overflowing)}
			>
				<div ref={innerRef}>{children}</div>
			</div>
			{(folded || expanded) && (
				<button
					className={classes("text-12", styles.toggle)}
					onClick={() => setExpanded(!expanded)}
					type="button"
				>
					{expanded ? "Show less" : "Show more"}
				</button>
			)}
		</>
	);
};
