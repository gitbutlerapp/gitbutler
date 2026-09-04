import type { Virtualizer } from "@tanstack/react-virtual";
import { type FC, useLayoutEffect, useState } from "react";
import { classes } from "#ui/components/classes.ts";
import styles from "./GraphRails.module.css";
import { legX, railPaths, railXFor, type GutterPlan, type Rail } from "./graph-layout.ts";

/** Frames to keep measuring while the rows' inset is in transition: well past its 180ms. */
const FOLLOW_FRAMES = 40;
/** A card's rail stub below its last row's glyph; keep in sync with StackCard.module.css. */
const CARD_STUB = 8;

/*
 * One SVG behind the stacks and the upstream section, drawing the rails
 * between them. Card anchors come from the virtualiser's measurements, so
 * cards that are scrolled out of the DOM still get their rails; upstream
 * anchors are measured from the section's rows. A ResizeObserver on the
 * content re-runs the measurement when a card's height lands or a fold
 * animates, so the rails follow either.
 * The rail's x is read off the header row, whose inset transitions when a
 * line comes or goes or the header moves onto the leg, and measuring goes
 * on frame by frame until it reads where the plan put it, so the rails
 * move with the glyphs instead of jumping ahead or freezing midway.
 */

export const GraphRails: FC<{
	plan: GutterPlan;
	/** The cards' virtualiser: one item per card, in plan order. */
	virtualizer: Virtualizer<HTMLDivElement, Element>;
	/**
	 * The positioned content box the SVG covers and positions are relative
	 * to, and the upstream section in it. Elements, not refs: an ancestor's
	 * ref attaches only after this component's layout effect has run on
	 * mount, so a ref read there is null until something else re-runs it.
	 */
	content: HTMLDivElement | null;
	section: HTMLDivElement | null;
}> = ({ plan, virtualizer, content, section }) => {
	const [rails, setRails] = useState<Array<Rail>>([]);
	const [height, setHeight] = useState(0);

	useLayoutEffect(() => {
		if (!content || !section) return;

		/** Measures once; true when the rail read off the rows is where the plan has it. */
		const measure = (): boolean => {
			const origin = content.getBoundingClientRect();
			const edgesOf = (element: Element): { y: number; topY: number; bottomY: number } => {
				const rect = element.getBoundingClientRect();
				const topY = rect.top - origin.top;
				return { y: topY + rect.height / 2, topY, bottomY: topY + rect.height };
			};
			const headerEl = section.querySelector("[data-graph-header]");
			if (headerEl === null) return true;
			// The header's glyph sits on the leg's line when the target has moved
			// on, and on the trunk otherwise: either way its live inset says
			// where the rail is right now.
			const headerRail = railXFor(Number.parseFloat(getComputedStyle(headerEl).paddingInlineStart));
			const railX = plan.header.incoming > 0 ? headerRail - (legX(plan) - plan.railX) : headerRail;
			const live = Number.isFinite(railX) ? { ...plan, railX } : plan;
			// What a fold hides is clipped away, and its measurements mean nothing.
			const inFold = (open: boolean, selector: string): Element | null =>
				open ? section.querySelector(selector) : null;
			// The leg's card, for the trunk rail to pass behind, and its rows, for
			// the leg's line to fork into and out of.
			const legCard = inFold(
				plan.incomingExpanded,
				"[data-graph-leg-card]",
			)?.getBoundingClientRect();
			const legRows = inFold(plan.incomingExpanded, "[data-graph-leg]");
			const legFirst = legRows?.firstElementChild?.getBoundingClientRect();
			const legLast = legRows?.lastElementChild?.getBoundingClientRect();
			const leg =
				legCard && legFirst && legLast
					? {
							card: { topY: legCard.top - origin.top, bottomY: legCard.bottom - origin.top },
							rows: { topY: legFirst.top - origin.top, bottomY: legLast.bottom - origin.top },
						}
					: null;
			const baseEl = section.querySelector("[data-graph-base-header]");
			const base = baseEl ? edgesOf(baseEl) : null;
			const baseRows = inFold(plan.baseExpanded, "[data-graph-base-rows]");
			const baseRowsTopY = baseRows?.hasChildNodes()
				? baseRows.getBoundingClientRect().top - origin.top
				: null;
			const endY = content.getBoundingClientRect().height;
			// A card the cache has no entry for yet waits; its measurement will resize the content.
			const cards = plan.order.map((_, index) => {
				const item = virtualizer.measurementsCache[index];
				return item !== undefined && Number.isFinite(item.end)
					? { topY: item.start, exitY: item.end - CARD_STUB, bottomY: item.end }
					: null;
			});
			const next = railPaths(live, {
				cards,
				cardsEnd: virtualizer.getTotalSize(),
				header: edgesOf(headerEl),
				leg,
				base,
				baseRowsTopY,
			});
			// Same paths, same state: skip the render.
			setRails((previous) =>
				previous.length === next.length &&
				previous.every((rail, index) => {
					const other = next[index];
					return other !== undefined && rail.d === other.d && rail.through === other.through;
				})
					? previous
					: next,
			);
			setHeight(endY);
			// Exact: the transition ends on the plan's value, and stopping short leaves a hair behind.
			return Math.abs(live.railX - plan.railX) < 0.001;
		};

		let frame = 0;
		let framesLeft = 0;
		const follow = () => {
			if (!measure() && framesLeft-- > 0) frame = requestAnimationFrame(follow);
		};
		const start = () => {
			framesLeft = FOLLOW_FRAMES;
			follow();
		};
		start();
		// Measuring in the next frame keeps the observer's own layout reads and
		// the state they set out of the frame that delivered the resize.
		const observer = new ResizeObserver(() => {
			cancelAnimationFrame(frame);
			frame = requestAnimationFrame(start);
		});
		observer.observe(section);
		observer.observe(content);
		return () => {
			observer.disconnect();
			cancelAnimationFrame(frame);
		};
	}, [plan, virtualizer, content, section]);

	return (
		<svg className={styles.rails} width="100%" height={height} aria-hidden>
			{rails.map((rail, index) => (
				<path
					// oxlint-disable-next-line react/no-array-index-key -- Every measure replaces the whole list.
					key={index}
					className={classes(styles.lane, rail.through && styles.through)}
					d={rail.d}
				/>
			))}
		</svg>
	);
};
