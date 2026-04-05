<!--
	VirtualList - A high-performance virtual scrolling component

	This component renders large lists efficiently by only rendering items that are
	currently visible in the viewport, making it ideal for displaying thousands of
	items without performance degradation.

	## Usage Example
	```svelte
	<VirtualList
		items={messages}
		defaultHeight={50}
		stickToBottom={true}
		onloadmore={loadMoreMessages}
	>
		{#snippet template(message)}
			<MessageRow {message} />
		{/snippet}
	</VirtualList>
	```

	@component
-->
<script lang="ts" module>
	type T = unknown;
</script>

<script lang="ts" generics="T">
	import Button from "$components/Button.svelte";
	import ScrollableContainer from "$components/scroll/ScrollableContainer.svelte";

	import { debounce } from "$lib/utils/debounce";

	import { resizeObserver } from "$lib/utils/resizeObserver";
	import { onDestroy, tick, untrack, type Snippet } from "svelte";
	import { fade } from "svelte/transition";
	import type { ScrollbarVisilitySettings } from "$components/scroll/Scrollbar.svelte";

	type Props = {
		/** Array of items to render in the virtual list. */
		items: Array<T>;
		/**
		 * Optional static content that is always rendered at the bottom of the list.
		 * Useful for input fields, footers, or persistent UI elements.
		 */
		children?: Snippet<[]>;
		/**
		 * Snippet template for rendering an item.
		 */
		template: Snippet<[T, number]>;
		/**
		 * Async callback triggered when scroll approaches the loading edge.
		 * When `stickToBottom` is true, fires near the TOP (for loading older content).
		 * Otherwise fires near the BOTTOM (for loading newer content).
		 * Also fires when content is shorter than viewport.
		 */
		onloadmore?: () => Promise<void>;
		/**
		 * Whether the list should grow to fill available space.
		 * Passed to the underlying ScrollableContainer.
		 */
		grow?: boolean;
		/**
		 * Initialize scroll position at bottom instead of top.
		 * Auto-scroll to bottom when new items are added and user is near bottom
		 * (within NEAR_BOTTOM_THRESHOLD px). Ideal for live feeds and chat interfaces.
		 */
		stickToBottom?: boolean;
		/**
		 * Scrollbar visibility settings.
		 * Options: 'scroll' (always), 'hover' (on hover), 'never'.
		 */
		visibility: ScrollbarVisilitySettings;
		/**
		 * Default height in pixels for each item before actual measurement.
		 * Used for initial layout calculation. More accurate values reduce layout shift.
		 */
		defaultHeight: number;
		/**
		 * Padding to apply to the scroll container.
		 * Useful for adding visual spacing around list content.
		 */
		padding?: {
			left?: number;
			right?: number;
			top?: number;
			bottom?: number;
		};
		/**
		 * The initial index to start rendering from when the list first loads.
		 * If provided, the list will initialize at this position instead of top or bottom.
		 * Takes precedence over stickToBottom for initial render.
		 */
		startIndex?: number;
		/**
		 * Extra pixels to render above and below the viewport.
		 * Higher values reduce flashing during fast scrolling at the cost of
		 * rendering more off-screen items. Defaults to 0.
		 */
		renderDistance?: number;
		/**
		 * Distance in pixels from the loading edge at which `onloadmore` fires.
		 * Defaults to 300.
		 */
		loadMoreThreshold?: number;
		/**
		 * Whether to show the "Scroll to bottom" button when user scrolls up.
		 * Defaults to false.
		 */
		showBottomButton?: boolean;
		/**
		 * Callback for when the visible items change. Note that this is not necessarily the
		 * same as the rendered range when `renderDistance !== 0`.
		 *
		 * Called with `undefined` when the component is destroyed, allowing consumers
		 * to clear any visible-range highlighting.
		 */
		onVisibleChange?: (change: { start: number; end: number } | undefined) => void;

		/** Returns a stable identifier for an item, used to detect head/tail changes. */
		getId: (item: T) => string | undefined;
		/**
		 * Optional static content rendered at the top of the scroll area, before
		 * any list items. Not tracked as a virtual list item.
		 */
		banner?: Snippet<[]>;
		/**
		 * Grace period in milliseconds to wait after rendering each item during
		 * initialization. Allows items to resize (e.g. async content like diffs
		 * expanding) before deciding whether more items fit in the viewport.
		 * This prevents eagerly rendering many items that will immediately be
		 * pushed out of range when the first item expands. Defaults to 0.
		 */
		initSettleMs?: number;
	};

	/** Distance from bottom (px) within which the user is considered "at the bottom". */
	const NEAR_BOTTOM_THRESHOLD = 70;
	const DEFAULT_LOAD_MORE_THRESHOLD = 300;
	const DEBOUNCE_DELAY = 50;
	const HEIGHT_LOCK_DURATION = 250;

	const {
		items,
		children,
		template,
		onloadmore,
		grow,
		padding,
		visibility,
		defaultHeight,
		stickToBottom,
		startIndex,
		renderDistance = 0,
		loadMoreThreshold = DEFAULT_LOAD_MORE_THRESHOLD,
		showBottomButton = false,
		onVisibleChange,
		getId,
		banner,
		initSettleMs = 0,
	}: Props = $props();

	let viewport = $state<HTMLDivElement>();
	let container = $state<HTMLDivElement>();
	// Live HTMLCollection — auto-updates as children change, no $state needed.
	let visibleRowElements: HTMLCollectionOf<Element> | undefined;

	// Height tracking — maps item index to measured pixel height
	let previousLength = 0;
	let previousHeadId: string | undefined;
	let previousTailId: string | undefined;
	let heightMap: number[] = [];
	let lockedHeights = $state<number[]>([]);
	let heightUnlockTimeouts: number[] = [];
	let viewportHeight = $state(0);
	let previousViewportHeight = 0;

	// Range tracking — which items are rendered vs visible in viewport
	let renderRange = $state({ start: 0, end: 0 });
	let visibleRange = $state({ start: 0, end: 0 });

	// Scroll tracking
	let offset = $state({ top: 0, bottom: 0 });
	let previousDistance = $state(0);
	let hasNewItemsAtBottom = $state(false);
	let isRecalculating = false;
	let isInitializing = false;
	// Incremented each time initializeAt is called; stale inits detect the
	// mismatch after each await and bail out.
	let initGeneration = 0;
	let lastScrollTop: number | undefined = undefined;
	let lastScrollDirection: "up" | "down" | undefined;
	let lastJumpToIndex: number | undefined;
	// True while the startIndex position-hold is active. Cleared once the
	// user scrolls manually, so resizes no longer snap back to startIndex.
	let holdStartIndex = false;
	// Suppresses the next onscroll so programmatic scrollTo calls don't
	// trigger recalculation or direction-tracking side effects.
	let skipNextScrollEvent = false;
	// Callback for the ResizeObserver to notify settle() that an item resized.
	// Includes the generation so stale timeouts don't clobber a newer settle.
	let initSettleNotify: { index: number; generation: number; resolve: () => void } | undefined;
	let initSettleTimeout: number | undefined;

	const debouncedLoadMore = $derived(debounce(() => onloadmore?.(), DEBOUNCE_DELAY));

	const renderItems = $derived(items.slice(renderRange.start, renderRange.end));

	function isInitialized() {
		return renderRange.end !== 0;
	}

	/**
	 * Waits for the ResizeObserver to report a height change on the given item,
	 * or falls back to `initSettleMs` timeout if no resize occurs.
	 * Resolves immediately when the observer fires, so items that resize fast
	 * don't block initialization.
	 */
	function settle(index: number, measuredHeight: number): Promise<void> {
		if (initSettleMs <= 0) return Promise.resolve();

		// If the measured height already differs significantly from
		// defaultHeight, the item rendered its final content directly
		// (skipping the skeleton/loading placeholder). No async expansion
		// is expected, so we can proceed immediately.
		if (Math.abs(measuredHeight - defaultHeight) > 1) {
			return Promise.resolve();
		}

		const gen = initGeneration;
		return new Promise((resolve) => {
			function done() {
				clearTimeout(initSettleTimeout);
				// Only clear the global notify if it still belongs to this settle.
				if (initSettleNotify?.generation === gen) {
					initSettleNotify = undefined;
					initSettleTimeout = undefined;
				}
				resolve();
			}
			initSettleTimeout = window.setTimeout(done, initSettleMs);
			initSettleNotify = { index, generation: gen, resolve: done };
		});
	}

	function getItemHeight(index: number): number {
		return heightMap[index] || defaultHeight;
	}

	function calculateHeightSum(startIndex: number, endIndex: number): number {
		let sum = 0;
		for (let i = startIndex; i < endIndex; i++) {
			sum += getItemHeight(i);
		}
		return sum;
	}

	function getDistanceFromBottom(): number {
		if (!viewport) return 0;
		return viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
	}

	function isNearBottom(distance: number): boolean {
		return distance < NEAR_BOTTOM_THRESHOLD;
	}

	function updateScrollDirection(): void {
		if (!viewport) return;
		const scrollTop = viewport.scrollTop;
		if (lastScrollTop !== undefined && lastScrollTop > scrollTop) {
			lastScrollDirection = "up";
		} else if (lastScrollTop !== undefined && lastScrollTop < scrollTop) {
			lastScrollDirection = "down";
		} else {
			lastScrollDirection = undefined;
		}
		lastScrollTop = scrollTop;
	}

	function shouldTriggerLoadMore(): boolean {
		if (!viewport || !onloadmore) return false;
		if (viewport.scrollHeight <= viewport.clientHeight) return true;
		if (stickToBottom) {
			// In stick-to-bottom mode, load more when near the TOP (older content)
			return viewport.scrollTop < loadMoreThreshold;
		}
		// In normal mode, load more when near the BOTTOM (newer content)
		const distance = getDistanceFromBottom();
		return distance >= 0 && distance < loadMoreThreshold;
	}

	/**
	 * Pins a row's CSS height to its cached value for HEIGHT_LOCK_DURATION ms.
	 * Prevents layout shift when items enter/exit the render range — the locked
	 * height holds the space while the item re-renders its content.
	 */
	function lockRowHeight(index: number): void {
		const cachedHeight = heightMap[index];
		if (!cachedHeight) return;

		lockedHeights[index] = cachedHeight;
		clearTimeout(heightUnlockTimeouts[index]);
		heightUnlockTimeouts[index] = window.setTimeout(() => {
			delete lockedHeights[index];
			delete heightUnlockTimeouts[index];
		}, HEIGHT_LOCK_DURATION);
	}

	// Tracks the item count when offsets were last fully recomputed.
	// The incremental offset update in recalculateRanges is only valid when
	// the item count hasn't changed since the last full recompute.
	let lastOffsetItemCount = 0;

	/**
	 * Sets top/bottom padding to represent the total height of items outside
	 * the render range. This keeps the scrollbar size and position accurate
	 * even though most items are not in the DOM.
	 */
	function updateOffsets() {
		offset = {
			top: calculateHeightSum(0, renderRange.start),
			bottom: calculateHeightSum(renderRange.end, heightMap.length),
		};
		lastOffsetItemCount = heightMap.length;
	}

	/**
	 * Calculates both render and visible ranges by scanning from the current
	 * renderRange.start (using offset.top as the pre-accumulated height) instead
	 * of from index 0. This reduces per-scroll work from O(totalItems) to
	 * O(visibleItems + scrollDelta). Scans backward when scrolling above the
	 * current render start.
	 */
	function calculateVisibleRange(): {
		renderStart: number;
		renderEnd: number;
		visibleStart: number;
		visibleEnd: number;
	} {
		const count = items.length;
		if (count === 0 || !viewport) {
			return { renderStart: 0, renderEnd: 0, visibleStart: 0, visibleEnd: 0 };
		}

		const scrollTop = viewport.scrollTop;
		const viewBottom = scrollTop + viewport.clientHeight;
		const renderTop = scrollTop - renderDistance;

		// Start scanning from the current render range start, using the
		// already-computed offset.top as the accumulated height up to that point.
		let scanStart = renderRange.start;
		let accumulatedHeight = offset.top;

		// If the viewport has scrolled above the current render start,
		// walk backward to find the new starting point.
		if (accumulatedHeight > renderTop && scanStart > 0) {
			for (let i = scanStart - 1; i >= 0; i--) {
				accumulatedHeight -= getItemHeight(i);
				scanStart = i;
				if (accumulatedHeight <= renderTop) break;
			}
		}

		let renderStart = -1;
		let visibleStart = -1;
		let visibleEnd = -1;

		// Forward scan: find render start, visible start/end, and render end.
		for (let i = scanStart; i < count; i++) {
			// Only read live DOM heights for items within the current render range.
			const inRenderRange = i >= renderRange.start && i < renderRange.end;
			const rowHeight = inRenderRange
				? visibleRowElements?.[i - renderRange.start]?.clientHeight || getItemHeight(i)
				: getItemHeight(i);
			accumulatedHeight += rowHeight;

			if (renderStart === -1 && accumulatedHeight >= renderTop) {
				renderStart = i;
			}
			if (visibleStart === -1 && accumulatedHeight >= scrollTop + 1) {
				visibleStart = i;
			}
			if (visibleEnd === -1 && accumulatedHeight > viewBottom) {
				visibleEnd = i + 1;
			}
			if (accumulatedHeight > viewBottom + renderDistance) {
				return { renderStart, renderEnd: i + 1, visibleStart, visibleEnd };
			}
		}

		// Content doesn't extend past viewport + renderDistance.
		return {
			renderStart: renderStart === -1 ? count - 1 : renderStart,
			renderEnd: count,
			visibleStart: visibleStart === -1 ? count - 1 : visibleStart,
			visibleEnd: visibleEnd === -1 ? count : visibleEnd,
		};
	}

	async function initializeAt(startingAt: number): Promise<void> {
		if (!viewport) return;
		isInitializing = true;
		// Cancel any outstanding settle from a previous init.
		clearTimeout(initSettleTimeout);
		initSettleNotify = undefined;
		initSettleTimeout = undefined;
		const generation = ++initGeneration;
		renderRange.start = startingAt;

		/** True if the rendered items fill the viewport (plus the given padding). */
		function isFull(padding: number): boolean {
			return (
				calculateHeightSum(renderRange.start, renderRange.end) > viewport!.clientHeight + padding
			);
		}

		try {
			// Position at an estimated offset so items don't flash at the stale
			// scrollTop. bottom padding must be ≥ viewportHeight so the browser
			// doesn't clamp the scrollTop we're about to set.
			const estimatedOffset = calculateHeightSum(0, startingAt);
			offset = { top: estimatedOffset, bottom: viewport.clientHeight };
			await tick();
			if (!viewport || generation !== initGeneration) return;
			viewport.scrollTop = estimatedOffset;

			// Forward fill: render items from startingAt until the viewport is full.
			for (let i = startingAt; i < items.length; i++) {
				if (heightMap[i] !== undefined) lockRowHeight(i);
				renderRange.end = i + 1;
				await tick();
				if (!viewport || generation !== initGeneration) return;
				const element = visibleRowElements?.[i - startingAt];
				if (!element) {
					console.warn("[VList:init] element missing after tick, aborting");
					return;
				}
				const measuredHeight = element.clientHeight;
				heightMap[i] = measuredHeight;

				if (isFull(renderDistance)) break;

				// Wait for async resize before deciding if the viewport is full.
				await settle(i, measuredHeight);
				if (!viewport || generation !== initGeneration) return;
				if (heightMap[i] !== measuredHeight && isFull(renderDistance)) break;
			}

			// Backward fill: render items above startingAt if space remains.
			for (let i = startingAt - 1; i >= 0; i--) {
				if (heightMap[i] !== undefined) lockRowHeight(i);

				// Shrink offset.top before the item renders so both changes
				// land in the same tick — no visual flash.
				const expectedHeight = lockedHeights[i] || defaultHeight;
				offset = { top: offset.top - expectedHeight, bottom: offset.bottom };
				renderRange.start = i;
				await tick();
				if (!viewport || generation !== initGeneration) return;
				const element = visibleRowElements?.[0];
				if (!element) {
					console.warn("[VList:init] element missing after tick, aborting");
					return;
				}

				// Compensate scrollTop for estimate vs actual height difference.
				const measuredHeight = element.clientHeight;
				const heightDiff = measuredHeight - expectedHeight;
				if (heightDiff !== 0) viewport.scrollTop += heightDiff;
				heightMap[i] = measuredHeight;

				if (isFull(2 * renderDistance)) break;

				await settle(i, measuredHeight);
				if (!viewport || generation !== initGeneration) return;

				// Item is above startingAt — its growth pushes content down.
				const settleDiff = heightMap[i] - measuredHeight;
				if (settleDiff !== 0) viewport.scrollTop += settleDiff;

				if (isFull(2 * renderDistance)) break;
			}

			if (!isInitialized() || generation !== initGeneration) return;

			updateOffsets();
			await tick();
			if (!viewport || generation !== initGeneration) return;

			const targetScrollTop = calculateHeightSum(0, startingAt);
			viewport.scrollTop = targetScrollTop;
			lastScrollTop = viewport.scrollTop;
		} finally {
			if (generation === initGeneration) {
				isInitializing = false;
				// Establish visibleRange, previousDistance, and fire loadMore
				// since scroll events were suppressed during init.
				await recalculateRanges();
			}
		}
	}

	async function recalculateRanges(): Promise<void> {
		if (!viewport || !visibleRowElements || !isInitialized() || isRecalculating || isInitializing)
			return;

		isRecalculating = true;

		const oldStart = renderRange.start;
		const oldEnd = renderRange.end;
		const bottomDistance = getDistanceFromBottom();

		const {
			renderStart: start,
			renderEnd: end,
			visibleStart,
			visibleEnd,
		} = calculateVisibleRange();

		if (start !== renderRange.start || end !== renderRange.end) {
			if (end <= start && items.length > 0) {
				// Degenerate range — preserve current state to avoid losing initialization.
				isRecalculating = false;
				return;
			}
			renderRange = { start, end };

			// Incremental offset update: adjust by the delta between old and new
			// range boundaries. O(|delta|) instead of O(totalItems). Only safe
			// when item count hasn't changed since the last full recompute.
			if (heightMap.length === lastOffsetItemCount) {
				let newTop = offset.top;
				let newBottom = offset.bottom;
				if (start < oldStart) {
					newTop -= calculateHeightSum(start, oldStart);
				} else if (start > oldStart) {
					newTop += calculateHeightSum(oldStart, start);
				}
				if (end < oldEnd) {
					newBottom += calculateHeightSum(end, oldEnd);
				} else if (end > oldEnd) {
					newBottom -= calculateHeightSum(oldEnd, end);
				}
				offset = { top: newTop, bottom: newBottom };
			} else {
				updateOffsets();
			}

			// Lock heights for items entering the render range (below old range, then above)
			for (let i = start; i < Math.min(end, oldStart); i++) {
				if (heightMap[i]) lockRowHeight(i);
			}
			for (let i = Math.max(start, oldEnd); i < end; i++) {
				if (heightMap[i]) lockRowHeight(i);
			}

			await tick();

			// Synchronously measure items that just entered at the top and
			// compensate scrollTop before the browser paints. This preempts
			// the ResizeObserver (which fires one frame late) and prevents
			// visible jitter when measured heights differ from estimates.
			if (start < oldStart && viewport) {
				let heightDrift = 0;
				for (let i = start; i < Math.min(oldStart, end); i++) {
					const el = visibleRowElements?.[i - start];
					if (!el) continue;
					const measured = el.clientHeight;
					const estimated = heightMap[i] || defaultHeight;
					if (measured !== estimated) {
						heightDrift += measured - estimated;
						heightMap[i] = measured;
					}
				}
				if (heightDrift !== 0) {
					viewport.scrollTop += heightDrift;
					skipNextScrollEvent = true;
				}
			}
		}

		if (visibleStart !== visibleRange.start || visibleEnd !== visibleRange.end) {
			visibleRange = { start: visibleStart, end: visibleEnd };
		}

		// If we were exactly at the bottom before recalculating but drifted due
		// to range/offset changes, snap back.
		if (stickToBottom && bottomDistance === 0 && getDistanceFromBottom() !== 0) {
			scrollToBottom();
		}

		previousDistance = getDistanceFromBottom();
		isRecalculating = false;

		if (shouldTriggerLoadMore()) {
			debouncedLoadMore();
		}
	}

	export function scrollToBottom(): void {
		if (!viewport) return;
		const target = viewport.scrollHeight - viewport.clientHeight;
		viewport.scrollTo({ top: target, behavior: "instant" });
	}

	export async function jumpToIndex(index: number) {
		// Guard: items can be undefined during unmount due to reactive teardown.
		if (!viewport || !items || index < 0 || index > items.length - 1) {
			return;
		}
		lastScrollDirection = undefined;
		lastJumpToIndex = index;

		// Fast path: if the target is already within the render range, just
		// scroll to it without tearing down and rebuilding the DOM.
		if (isInitialized() && index >= renderRange.start && index < renderRange.end) {
			const target = offset.top + calculateHeightSum(renderRange.start, index);
			skipNextScrollEvent = true;
			viewport.scrollTop = target;
			await recalculateRanges();
			return;
		}

		lockRowHeight(index);
		await initializeAt(index);
	}

	/**
	 * Updates heightMap when items change: resets on full replacement,
	 * shifts on prepend, or adjusts length for appends/removals.
	 */
	function updateHeightMap(countDelta: number, headChanged: boolean, tailChanged: boolean): void {
		if (headChanged && tailChanged && previousLength > 0) {
			// Items completely replaced (e.g., switching views) — clear stale
			// heights and renderRange so isInitialized() returns false below.
			heightMap = new Array(items.length);
			renderRange = { start: 0, end: 0 };
		} else if (headChanged && !tailChanged && countDelta > 0) {
			// Prepend: shift cached heights to match new indices.
			const shifted: number[] = new Array(items.length);
			for (let i = 0; i < previousLength; i++) {
				if (heightMap[i] !== undefined) {
					shifted[i + countDelta] = heightMap[i]!;
				}
			}
			heightMap = shifted;
		} else {
			heightMap.length = items.length;
		}
	}

	async function handleItemsAdded(
		countDelta: number,
		headChanged: boolean,
		tailChanged: boolean,
	): Promise<void> {
		if (stickToBottom && headChanged && !tailChanged && viewport) {
			// HEAD prepend: shift renderRange to keep the same items visible,
			// then compensate scroll position for the new content above.
			renderRange = {
				start: renderRange.start + countDelta,
				end: Math.min(renderRange.end + countDelta, items.length),
			};
			updateOffsets();
			await tick();
			const compensation = countDelta * defaultHeight;
			viewport.scrollBy({ top: compensation });
			await recalculateRanges();
		} else if (stickToBottom && tailChanged && !headChanged) {
			const distance = getDistanceFromBottom();
			if (isNearBottom(previousDistance) || isNearBottom(distance)) {
				// When more items arrive than are currently rendered, the incremental
				// path would render at the old scrollTop (top of list) then scroll down,
				// causing a flash and potential drift from height estimate errors.
				// Re-initializing from the bottom builds the correct render range directly.
				const renderedCount = renderRange.end - renderRange.start;
				if (countDelta > renderedCount) {
					renderRange = { start: 0, end: 0 };
					await initializeAt(items.length - 1);
					scrollToBottom();
					return;
				}
				await tick();
				await recalculateRanges();
				scrollToBottom();
			} else {
				hasNewItemsAtBottom = true;
			}
		}
		updateOffsets();
	}

	async function handleItemsRemovedOrReplaced(): Promise<void> {
		await tick();
		if (stickToBottom && isNearBottom(previousDistance)) {
			await recalculateRanges();
			if (getDistanceFromBottom() !== 0) {
				scrollToBottom();
			}
		}
	}

	async function handleItemsChanged(
		countDelta: number,
		headChanged: boolean,
		tailChanged: boolean,
	): Promise<void> {
		updateHeightMap(countDelta, headChanged, tailChanged);

		if (!isInitialized() && items.length > 0) {
			const initAt = stickToBottom ? items.length - 1 : startIndex || 0;
			holdStartIndex = !!startIndex;
			await initializeAt(initAt);
			if (stickToBottom) {
				scrollToBottom();
			}
			return;
		}

		if (countDelta > 0) {
			await handleItemsAdded(countDelta, headChanged, tailChanged);
		} else {
			await handleItemsRemovedOrReplaced();
		}
	}

	// ── Observers ────────────────────────────────────────────────────────

	/**
	 * Compensates scroll position when an observed item resizes, preventing
	 * visual jumps during scrolling, jumping, or stick-to-bottom behavior.
	 */
	function compensateScrollForResize(index: number, oldHeight: number): void {
		if (!viewport) return;

		if (
			lastScrollDirection === "up" &&
			offset.top !== viewport.scrollTop &&
			renderRange.start === index
		) {
			const compensation = heightMap[index] - oldHeight;
			if (compensation === 0) return;
			// When scrolling up, compensate for the height change of the topmost
			// visible element to prevent content from jumping downward.
			viewport.scrollBy({ top: compensation });
		} else if (
			(lastJumpToIndex !== undefined || holdStartIndex) &&
			lastScrollDirection === undefined &&
			!isInitializing
		) {
			// Maintain position as off-viewport elements resize after a jump.
			// Skip during init — heightMap is incomplete.
			const target = calculateHeightSum(0, lastJumpToIndex ?? startIndex ?? 0);
			// Subpixel tolerance: scrollTop is fractional, heightSum is integer.
			if (Math.abs(viewport.scrollTop - target) < 1) return;
			viewport.scrollTop = target;
			skipNextScrollEvent = true;
		} else if (isNearBottom(previousDistance) && lastScrollDirection !== "up") {
			// Snap back to bottom when stickToBottom drifted >2px, or the last
			// item resized. The >2px guard prevents subpixel oscillation.
			const shouldSnap =
				(stickToBottom && getDistanceFromBottom() > 2) ||
				(index === items.length - 1 && viewport.scrollTop !== 0);
			if (shouldSnap) {
				skipNextScrollEvent = true;
				scrollToBottom();
			}
		}
	}

	const itemObserver = new ResizeObserver((entries) => {
		if (!viewport) return;
		let shouldRecalculate = false;
		for (const entry of entries) {
			const { target } = entry;
			if (!target.isConnected) continue;
			if (!(target instanceof HTMLElement)) continue;

			shouldRecalculate = true;

			const indexAttr = target.dataset.index;
			const index = indexAttr ? parseInt(indexAttr) : undefined;
			if (index !== undefined) {
				const oldHeight = getItemHeight(index);
				if (target.clientHeight !== heightMap[index]) {
					heightMap[index] = target.clientHeight;
					// During initialization, notify settle() that this item resized.
					if (initSettleNotify?.index === index) {
						initSettleNotify.resolve();
					}
				}
				compensateScrollForResize(index, oldHeight);
			}
		}

		if (shouldRecalculate) {
			recalculateRanges();
		}
	});

	const mutationObserver = new MutationObserver((mutations) => {
		for (const mutation of mutations) {
			for (const node of mutation.addedNodes) {
				if (node instanceof HTMLElement && node.matches("[data-index]")) {
					itemObserver.observe(node);
				}
			}
			for (const node of mutation.removedNodes) {
				if (node instanceof HTMLElement) {
					itemObserver.unobserve(node);
				}
			}
		}
	});

	// ── Effects ──────────────────────────────────────────────────────────

	// Wire up observers when the container mounts.
	$effect(() => {
		if (container) {
			mutationObserver.observe(container, { childList: true });
			for (const el of container.querySelectorAll(":scope > [data-index]")) {
				itemObserver.observe(el);
			}
		}
		return () => {
			mutationObserver.disconnect();
			itemObserver.disconnect();
		};
	});

	// getElementsByClassName returns a live HTMLCollection that auto-updates
	// as the DOM changes, so we only need to grab it once per viewport.
	$effect(() => {
		if (!viewport) return;
		visibleRowElements = viewport.getElementsByClassName("list-row");
	});

	// When the viewport resizes, recalculate ranges. If stickToBottom is
	// active and the user was near the bottom, snap back to bottom
	// regardless of whether the viewport shrank or grew.
	$effect(() => {
		if (!viewportHeight) return;
		const shrunk = viewportHeight < previousViewportHeight;
		const grew = viewportHeight > previousViewportHeight;
		previousViewportHeight = viewportHeight;
		if (shrunk || grew) {
			untrack(async () => {
				const nearBottom = isNearBottom(previousDistance);
				await recalculateRanges();
				if (stickToBottom && nearBottom) {
					scrollToBottom();
				}
			});
		}
	});

	// React to items array changes. Reads items/getId inside the effect for
	// Svelte tracking, then delegates to handleItemsChanged via untrack to
	// avoid re-triggering on the state mutations that handler makes.
	$effect(() => {
		if (!viewport) return;
		if (!items || items.length === 0) {
			if (previousLength > 0) {
				heightMap = [];
				renderRange = { start: 0, end: 0 };
				offset = { top: 0, bottom: 0 };
				previousLength = 0;
				previousHeadId = undefined;
				previousTailId = undefined;
				lastOffsetItemCount = 0;
				for (const t of heightUnlockTimeouts) clearTimeout(t);
				heightUnlockTimeouts = [];
				lockedHeights = [];
			}
			// Empty list with viewport — content is shorter than viewport,
			// so trigger onloadmore if available.
			untrack(() => {
				if (shouldTriggerLoadMore()) {
					debouncedLoadMore();
				}
			});
			return;
		}
		const currentHeadId = getId(items[0]);
		const currentTailId = getId(items.at(-1)!);
		const countDelta = items.length - previousLength;
		const headChanged = previousLength === 0 || currentHeadId !== previousHeadId;
		const tailChanged = previousLength === 0 || currentTailId !== previousTailId;

		untrack(() => handleItemsChanged(countDelta, headChanged, tailChanged));
		previousLength = items.length;
		previousHeadId = currentHeadId;
		previousTailId = currentTailId;
	});

	// Clear "new items at bottom" indicator once the user scrolls far enough
	// that the render range reaches the end of the list.
	$effect(() => {
		if (hasNewItemsAtBottom && renderRange.end === items.length) {
			hasNewItemsAtBottom = false;
		}
	});

	// Forward visible range to consumer callback.
	$effect(() => {
		onVisibleChange?.(visibleRange);
	});

	// Signal undefined on destroy so consumers can clear highlights.
	onDestroy(() => {
		onVisibleChange?.(undefined);
	});
</script>

<ScrollableContainer
	bind:viewportHeight
	bind:viewport
	zIndex="3"
	onscroll={() => {
		if (!viewport) return;
		// During initialization, ignore all scroll events — init manages its
		// own scroll positioning and heightMap is incomplete.
		if (isInitializing) return;
		if (skipNextScrollEvent) {
			skipNextScrollEvent = false;
			// If the scroll moved less than half the viewport, it's a small
			// compensation — skip it. Large jumps override a potentially stale flag.
			const delta = Math.abs(viewport.scrollTop - (lastScrollTop ?? 0));
			if (delta < viewport.clientHeight / 2) {
				return;
			}
		}
		updateScrollDirection();
		// Once the user scrolls manually, stop holding the jump/start position.
		if (lastScrollDirection !== undefined) {
			lastJumpToIndex = undefined;
			holdStartIndex = false;
		}
		recalculateRanges();
	}}
	wide={grow}
	whenToShow={visibility}
	{padding}
>
	{@render banner?.()}
	<div
		bind:this={container}
		data-remove-from-panning
		class="padded-contents"
		style:padding-top={offset.top + "px"}
		style:padding-bottom={offset.bottom + "px"}
	>
		{#each renderItems as data, i (renderRange.start + i)}
			{@const absIndex = renderRange.start + i}
			{@const locked = lockedHeights[absIndex]}
			<div class="list-row" data-index={absIndex} style:height={locked ? `${locked}px` : undefined}>
				{@render template(data, absIndex)}
			</div>
		{/each}
		<div
			class="children"
			use:resizeObserver={() => {
				if (
					stickToBottom &&
					isNearBottom(previousDistance) &&
					lastScrollDirection !== "up" &&
					getDistanceFromBottom() > 2
				) {
					scrollToBottom();
				}
			}}
		>
			{@render children?.()}
		</div>
	</div>
	{#snippet actions()}
		{#if previousDistance > NEAR_BOTTOM_THRESHOLD || hasNewItemsAtBottom}
			<div class="feed-actions">
				{#if hasNewItemsAtBottom}
					<button
						type="button"
						class="text-12 feed-actions__new-messages"
						transition:fade={{ duration: 150 }}
						onclick={() => {
							jumpToIndex(items.length - 1);
						}}
					>
						New unread
					</button>
				{/if}
				{#if showBottomButton && previousDistance > NEAR_BOTTOM_THRESHOLD}
					<div class="feed-actions__scroll-to-bottom" transition:fade={{ duration: 150 }}>
						<Button
							kind="outline"
							icon="arrow-down"
							tooltip="Scroll to bottom"
							onclick={() => {
								if (items && items.length > 0) {
									scrollToBottom();
								}
							}}
						/>
					</div>
				{/if}
			</div>
		{/if}
	{/snippet}
</ScrollableContainer>

<style>
	.list-row {
		display: block;
	}

	.padded-contents {
		display: flex;
		flex-direction: column;
	}

	.feed-actions {
		display: flex;
		z-index: var(--z-floating);
		position: absolute;
		right: 16px;
		bottom: 14px;
		gap: 4px;
	}

	.feed-actions__new-messages {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 4px 8px;
		border: 1px solid var(--border-3);
		border-radius: var(--radius-button);
		background-color: var(--bg-2);
		color: var(--text-1);
	}

	.feed-actions__scroll-to-bottom {
		z-index: var(--z-floating);
		overflow: hidden;
		border-radius: var(--radius-button);
		background-color: var(--bg-1);
		transition:
			box-shadow var(--transition-fast),
			transform var(--transition-medium);

		&:hover {
			transform: scale(1.05) translateY(-2px);
			box-shadow: var(--fx-shadow-s);
		}
	}
</style>
