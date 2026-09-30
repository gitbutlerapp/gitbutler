import { ScrollArea as BaseScrollArea } from "@base-ui/react";
import {
	useSyncExternalStore,
	type ComponentProps,
	type CSSProperties,
	type FC,
	type PointerEvent,
	type Ref,
} from "react";
import { classes } from "./classes.ts";
import styles from "./ScrollArea.module.css";

/**
 * A pane's scroll container, with a thin scrollbar drawn over the content rather than a gutter
 * beside it. The thumb shows while the area is scrolled or under the pointer, and fades away at
 * rest.
 *
 * Two elements, so their classes go to the right one: `className` places the area in the layout
 * that holds it (its growth, its height, a border), and `viewportClassName` lays out what scrolls
 * inside it (a flex column, padding). The viewport is the element that scrolls: give a virtualizer,
 * or anything that measures or restores scrolling, `viewportRef`.
 *
 * `separator` draws a hairline across the top while the content is scrolled down, so the rows
 * passing under a sticky header read as passing under it.
 *
 * @public
 * @import import { ScrollArea } from "@gitbutler/ui-react/ScrollArea.tsx";
 */
export const ScrollArea: FC<
	{
		viewportClassName?: string;
		viewportRef?: Ref<HTMLDivElement>;
		separator?: boolean;
	} & ComponentProps<"div">
> = ({ viewportClassName, viewportRef, separator = false, className, children, ...props }) => (
	<BaseScrollArea.Root
		{...props}
		className={classes(className, styles.root, separator && styles.separator)}
	>
		<BaseScrollArea.Viewport
			ref={viewportRef}
			className={classes(viewportClassName, styles.viewport)}
		>
			{/* Watched for size, so the thumb follows content that grows or swaps without a scroll or
			    a resize of the viewport: another page's content, a list that loads. */}
			<BaseScrollArea.Content className={styles.content}>{children}</BaseScrollArea.Content>
		</BaseScrollArea.Viewport>
		<BaseScrollArea.Scrollbar orientation="vertical" className={styles.scrollbar}>
			<BaseScrollArea.Thumb className={styles.thumb} onPointerMove={ignoreButtonlessDrag} />
		</BaseScrollArea.Scrollbar>
		<BaseScrollArea.Scrollbar orientation="horizontal" className={styles.scrollbar}>
			<BaseScrollArea.Thumb className={styles.thumb} onPointerMove={ignoreButtonlessDrag} />
		</BaseScrollArea.Scrollbar>
	</BaseScrollArea.Root>
);

/**
 * Base UI drags from a thumb's pointer-down to its pointer-up, and a pointer-up it misses leaves
 * the drag running: every later move over the thumb then scrolls back towards where the drag
 * began. A move with no button held is no drag, so it never reaches that handler.
 */
const ignoreButtonlessDrag = (
	event: Parameters<NonNullable<BaseScrollArea.Thumb.Props["onPointerMove"]>>[0],
) => {
	if (event.buttons === 0) event.preventBaseUIHandler();
};

/**
 * {@link ScrollArea}'s scrollbars for a scroller something else renders and owns, such as a
 * library's virtualized view that takes only a class name. They show and behave as the area's do.
 *
 * Place them in a positioned box that the scroller fills, as its sibling, and hide the scroller's
 * native scrollbar in its own CSS (`scrollbar-width: none`). Hand over the element itself, from a
 * ref callback into state, so the bars follow it when it mounts.
 *
 * @public
 * @import import { ScrollBars } from "@gitbutler/ui-react/ScrollArea.tsx";
 */
export const ScrollBars: FC<{ scrollElement: HTMLElement | null; className?: string }> = ({
	scrollElement,
	className,
}) => {
	const snapshot = useSyncExternalStore(
		scrollElement ? subscribeTo(scrollElement) : subscribeToNothing,
		() => (scrollElement ? readScroller(scrollElement) : null),
	);
	if (!scrollElement || snapshot === null) return null;

	const metrics = parseSnapshot(snapshot);
	return (
		<div className={classes(className, styles.bars)}>
			<Bar element={scrollElement} metrics={metrics} orientation="vertical" />
			<Bar element={scrollElement} metrics={metrics} orientation="horizontal" />
		</div>
	);
};

type Orientation = "vertical" | "horizontal";

type ScrollerMetrics = {
	vertical: Axis;
	horizontal: Axis;
	hovering: boolean;
	scrolling: boolean;
};

/** One axis of the scroller: how far it is scrolled, and its viewport and content lengths. */
type Axis = { offset: number; viewport: number; content: number };

/** The bar's padding, which the thumb keeps from both ends of the track. */
const trackInset = 2;
/** Base UI's own floor, so a thumb over a very long scroller stays big enough to grab. */
const minThumbLength = 16;
/** How long the bars stay after the last scroll, as the area's do. */
const scrollingLinger = 500;
/** How long a scroll after the user's input still counts as theirs, as Base UI's area measures it. */
const userScrollSettle = 100;

const Bar: FC<{ element: HTMLElement; metrics: ScrollerMetrics; orientation: Orientation }> = ({
	element,
	metrics,
	orientation,
}) => {
	const { offset, viewport, content } = metrics[orientation];
	if (content <= viewport) return null;

	const track = viewport - trackInset * 2;
	const thumb = Math.max(minThumbLength, (viewport / content) * track);
	const travel = track - thumb;
	const position = (offset / (content - viewport)) * travel;
	const vertical = orientation === "vertical";
	// Pixels of scrolling per pixel of thumb travel.
	const ratio = travel > 0 ? (content - viewport) / travel : 0;

	const scrollTo = (next: number) => {
		element.scrollTo(
			vertical ? { top: next, behavior: "instant" } : { left: next, behavior: "instant" },
		);
	};

	const onTrackPointerDown = (event: PointerEvent<HTMLDivElement>) => {
		if (event.target !== event.currentTarget || event.button !== 0) return;
		// Brings the thumb's middle to the pointer, as the area's track does.
		const rect = event.currentTarget.getBoundingClientRect();
		const along = vertical ? event.clientY - rect.top : event.clientX - rect.left;
		scrollTo((along - trackInset - thumb / 2) * ratio);
	};

	const onThumbPointerDown = (event: PointerEvent<HTMLDivElement>) => {
		if (event.button !== 0) return;
		event.preventDefault();
		const thumbElement = event.currentTarget;
		thumbElement.setPointerCapture(event.pointerId);
		const start = vertical ? event.clientY : event.clientX;
		const startOffset = offset;

		const onMove = (move: globalThis.PointerEvent) => {
			scrollTo(startOffset + ((vertical ? move.clientY : move.clientX) - start) * ratio);
		};
		const onEnd = () => {
			thumbElement.removeEventListener("pointermove", onMove);
			thumbElement.removeEventListener("lostpointercapture", onEnd);
		};
		thumbElement.addEventListener("pointermove", onMove);
		thumbElement.addEventListener("lostpointercapture", onEnd);
	};

	return (
		<div
			className={styles.scrollbar}
			data-orientation={orientation}
			data-hovering={metrics.hovering || undefined}
			data-scrolling={metrics.scrolling || undefined}
			onPointerDown={onTrackPointerDown}
			style={
				{
					[vertical ? "--scroll-area-thumb-height" : "--scroll-area-thumb-width"]: `${thumb}px`,
				} as CSSProperties
			}
		>
			<div
				className={styles.thumb}
				data-orientation={orientation}
				onPointerDown={onThumbPointerDown}
				style={{
					transform: vertical ? `translateY(${position}px)` : `translateX(${position}px)`,
				}}
			/>
		</div>
	);
};

/** Whether the pointer is over each watched scroller, and whether it has just scrolled. */
const scrollerStates = new WeakMap<HTMLElement, { hovering: boolean; scrolling: boolean }>();

/**
 * One subscription per scroller, the same function each time: a new one would have React
 * resubscribe on every draw, and resubscribing forgets that the scroller is mid-scroll.
 */
const subscriptions = new WeakMap<HTMLElement, (notify: () => void) => () => void>();

const subscribeTo = (element: HTMLElement) => {
	let subscribe = subscriptions.get(element);
	if (!subscribe) {
		subscribe = (notify) => watchScroller(element, notify);
		subscriptions.set(element, subscribe);
	}
	return subscribe;
};

const subscribeToNothing = () => () => {};

/**
 * Calls `notify` whenever the bars could need redrawing: the scroller scrolls, the pointer comes or
 * goes, or the scroller or anything directly in it changes size (a virtualized list grows its
 * content by resizing a child, which the scroller's own size never shows).
 */
/** What Base UI's area takes for the user moving the content: a scroll after one of these is theirs. */
const inputEvents = ["wheel", "touchmove", "pointermove", "keydown"] as const;

const watchScroller = (element: HTMLElement, notify: () => void): (() => void) => {
	const state = { hovering: element.matches(":hover"), scrolling: false };
	scrollerStates.set(element, state);
	let lingering: ReturnType<typeof setTimeout> | undefined;
	// Only a scroll the user drives shows the bars, as the area's do: the owner scrolling the
	// element itself, restoring a position or revealing a row, would flash them for nothing.
	let userDriven = false;
	let settling: ReturnType<typeof setTimeout> | undefined;
	const onInput = () => {
		userDriven = true;
	};

	const onScroll = () => {
		clearTimeout(settling);
		settling = setTimeout(() => {
			userDriven = false;
		}, userScrollSettle);
		if (!userDriven) {
			notify();
			return;
		}
		state.scrolling = true;
		clearTimeout(lingering);
		lingering = setTimeout(() => {
			state.scrolling = false;
			notify();
		}, scrollingLinger);
		notify();
	};
	const onEnter = () => {
		userDriven = true;
		state.hovering = true;
		notify();
	};
	const onLeave = () => {
		state.hovering = false;
		notify();
	};

	const resizes = new ResizeObserver(notify);
	const observeChildren = () => {
		resizes.disconnect();
		resizes.observe(element);
		for (const child of element.children) resizes.observe(child);
		notify();
	};
	const mutations = new MutationObserver(observeChildren);

	element.addEventListener("scroll", onScroll, { passive: true });
	element.addEventListener("pointerenter", onEnter);
	element.addEventListener("pointerleave", onLeave);
	for (const type of inputEvents) element.addEventListener(type, onInput, { passive: true });
	mutations.observe(element, { childList: true });
	observeChildren();

	return () => {
		clearTimeout(lingering);
		clearTimeout(settling);
		element.removeEventListener("scroll", onScroll);
		element.removeEventListener("pointerenter", onEnter);
		element.removeEventListener("pointerleave", onLeave);
		for (const type of inputEvents) element.removeEventListener(type, onInput);
		mutations.disconnect();
		resizes.disconnect();
		scrollerStates.delete(element);
	};
};

/** A string, so an unchanged scroller reads as the same snapshot and draws nothing new. */
const readScroller = (element: HTMLElement): string => {
	const state = scrollerStates.get(element);
	return [
		element.scrollTop,
		element.clientHeight,
		element.scrollHeight,
		element.scrollLeft,
		element.clientWidth,
		element.scrollWidth,
		state?.hovering ? 1 : 0,
		state?.scrolling ? 1 : 0,
	].join(" ");
};

const parseSnapshot = (snapshot: string): ScrollerMetrics => {
	const [
		top = 0,
		height = 0,
		contentHeight = 0,
		left = 0,
		width = 0,
		contentWidth = 0,
		hovering,
		scrolling,
	] = snapshot.split(" ").map(Number);
	return {
		vertical: { offset: top, viewport: height, content: contentHeight },
		horizontal: { offset: left, viewport: width, content: contentWidth },
		hovering: hovering === 1,
		scrolling: scrolling === 1,
	};
};
