import { getButtonClassName } from "#ui/components/Button.tsx";
import { classes } from "#ui/components/classes.ts";
import { Icon } from "#ui/components/Icon.tsx";
import type { CSSProperties, FC, MouseEvent, ReactNode } from "react";
import { useLayoutEffect, useRef, useState } from "react";
import styles from "./Clamped.module.css";

/**
 * Resolve the supported clamp lengths ("240px", "80vh", "3lh") to pixels.
 * `lh` reads the line-height off `text`, so it must be the element that
 * carries the content's text style; a `normal` line-height resolves to NaN,
 * which never folds.
 */
const resolveLength = (length: string, text: Element): number => {
	const match = /^(\d+(?:\.\d+)?)(px|vh|lh)$/.exec(length);
	if (match === null) return Number.POSITIVE_INFINITY;
	const value = Number(match[1]);
	if (match[2] === "vh") return (value * window.innerHeight) / 100;
	if (match[2] === "lh") return value * Number.parseFloat(getComputedStyle(text).lineHeight);
	return value;
};

/**
 * Caps content at `maxHeight` with a fade and a toggle, folding only when
 * content is actually taller than the cap (or than `foldOver`, when given).
 * Content that changes size after mount (lazy images, async syntax
 * highlighting) re-measures via a ResizeObserver on the inner wrapper, and
 * viewport resizes re-measure the vh-based caps.
 */
export const Clamped: FC<{
	/** A px, vh or lh length, e.g. `"240px"`, `"80vh"` or `"3lh"`. */
	maxHeight: string;
	/**
	 * Fold only once the content is taller than this, so that a fold never
	 * hides just a line or two behind a click. Defaults to `maxHeight`.
	 */
	foldOver?: string;
	/** Don't fold when the full content already fits within the viewport. */
	skipWhenViewportFits?: boolean;
	/**
	 * `text` (the default) seats a Show more link on the fade and a Show less
	 * link under the unfolded content. `card` sets the folded content on a
	 * quiet card with a chevron at its corner; unfolded, the content lies flat
	 * with the chevron at its top right, where the eye was left.
	 */
	variant?: "text" | "card";
	children: ReactNode;
}> = ({
	maxHeight,
	foldOver = maxHeight,
	skipWhenViewportFits = false,
	variant = "text",
	children,
}) => {
	const [expanded, setExpanded] = useState(false);
	const [folded, setFolded] = useState(false);
	// The cap in pixels, so the CSS clamp and the fold decision agree even
	// when `maxHeight` is in lh, which the outer box would otherwise resolve
	// against its own line-height rather than the content's.
	const [capPx, setCapPx] = useState<number | null>(null);
	const innerRef = useRef<HTMLDivElement | null>(null);

	useLayoutEffect(() => {
		if (expanded) return;
		const inner = innerRef.current;
		if (inner === null) return;

		// The inner wrapper always has its natural height (only the outer box
		// is clamped), so this needs no unfold-and-measure dance.
		const measure = () => {
			// The content's root (a Markdown block, say) carries the text style
			// the `lh` unit reads; the wrapper itself is unstyled.
			const text = inner.firstElementChild ?? inner;
			const contentHeight = inner.offsetHeight;
			const fitsViewport = skipWhenViewportFits && contentHeight <= window.innerHeight;
			setCapPx(resolveLength(maxHeight, text));
			// Only ever folds, never unfolds on its own: folding can narrow the
			// content (the card's inset, a page scrollbar coming or going), and a
			// measurement that could undo the fold would chase its own layout.
			if (contentHeight > resolveLength(foldOver, text) + 1 && !fitsViewport) setFolded(true);
		};
		measure();
		const observer = new ResizeObserver(measure);
		observer.observe(inner);
		window.addEventListener("resize", measure);
		return () => {
			observer.disconnect();
			window.removeEventListener("resize", measure);
		};
	}, [expanded, maxHeight, foldOver, skipWhenViewportFits]);

	const isFolded = !expanded && folded;

	const clamp = (
		<div
			style={
				{
					"--clamp-max-height":
						capPx === null || !Number.isFinite(capPx) ? maxHeight : `${capPx}px`,
				} as CSSProperties
			}
			className={classes(
				isFolded && styles.clamped,
				isFolded && styles.overflowing,
				variant === "card" && styles.cardContent,
			)}
		>
			<div ref={innerRef}>{children}</div>
			{/* Folded, the trigger rides on the fade instead of sitting under it,
			    so it costs the clamp no extra row. */}
			{isFolded && variant === "text" && (
				<button
					className={classes("text-13", "text-body", styles.toggle, styles.toggleOverlay)}
					onClick={() => setExpanded(true)}
					type="button"
				>
					Show more
				</button>
			)}
		</div>
	);

	if (variant === "card") {
		// The same tree in every state, with only classes and the chevron
		// changing: were the card wrapper added and removed around the content,
		// React would reuse the outer divs and remount the content, moving the
		// measured wrapper out from under the observer and flickering between
		// folded and flat. Short content simply gets no card and no chevron.
		const framed = isFolded || expanded;
		// Folded, the whole card is the target, not just its chevron. A link or
		// control inside keeps its own click, and a drag that selected text is
		// not a click on the card.
		const expandFromCard = (event: MouseEvent<HTMLDivElement>) => {
			if (!(event.target instanceof Element) || event.target.closest("a, button, summary")) return;
			if (window.getSelection()?.isCollapsed === false) return;
			setExpanded(true);
		};
		return (
			// oxlint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- a pointer convenience; the chevron button is the keyboard path.
			<div
				className={classes(styles.card, framed && styles.cardFramed, isFolded && styles.cardFolded)}
				onClick={isFolded ? expandFromCard : undefined}
			>
				{clamp}
				{framed && (
					<button
						aria-expanded={expanded}
						aria-label={expanded ? "Show less" : "Show more"}
						className={classes(
							getButtonClassName({ variant: "ghost", iconOnly: true, size: "small" }),
							styles.cardToggle,
						)}
						onClick={() => setExpanded(!expanded)}
						type="button"
					>
						<Icon name={expanded ? "chevron-up" : "chevron-down"} />
					</button>
				)}
			</div>
		);
	}

	return (
		<>
			{clamp}
			{expanded && (
				<button
					className={classes("text-13", "text-body", styles.toggle, styles.toggleInline)}
					onClick={() => setExpanded(false)}
					type="button"
				>
					Show less
				</button>
			)}
		</>
	);
};
