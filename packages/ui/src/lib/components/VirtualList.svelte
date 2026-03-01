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
	};

	/** Distance from bottom (px) within which the user is considered "at the bottom". */
	const NEAR_BOTTOM_THRESHOLD = 70;
	const LOAD_MORE_THRESHOLD = 300;
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
		showBottomButton = false,
		onVisibleChange,
		getId,
	}: Props = $props();

	let viewport = $state<HTMLDivElement>();
	let container = $state<HTMLDivElement>();
	// Live HTMLCollection — auto-updates as children change, no $state needed.
	let visibleRowElements: HTMLCollectionOf<Element> | undefined;

	// Height tracking — maps item index to measured pixel height
	let previousItems: T[] = [];
	let heightMap: number[] = $state([]);
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
	let lastScrollTop: number | undefined = undefined;
	let lastScrollDirection: "up" | "down" | undefined;
	let lastJumpToIndex: number | undefined;
	// Suppresses the next onscroll so programmatic scrollTo calls don't
	// trigger recalculation or direction-tracking side effects.
	let skipNextScrollEvent = false;

	const debouncedLoadMore = $derived(debounce(() => onloadmore?.(), DEBOUNCE_DELAY));

	const renderItems = $derived(items.slice(renderRange.start, renderRange.end));

	function isInitialized() {
		return renderRange.end !== 0;
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
			return viewport.scrollTop < LOAD_MORE_THRESHOLD;
		}
		// In normal mode, load more when near the BOTTOM (newer content)
		const distance = getDistanceFromBottom();
		return distance >= 0 && distance < LOAD_MORE_THRESHOLD;
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
	}

	/**
	 * Single-pass calculation of both render and visible ranges.
	 * Walks from item 0, accumulating heights, and records four boundaries:
	 *   renderStart  — first item within (scrollTop - renderDistance)
	 *   visibleStart — first item whose bottom edge crosses scrollTop
	 *   visibleEnd   — first item whose top edge crosses viewport bottom (exclusive)
	 *   renderEnd    — first item past (viewport bottom + renderDistance) (exclusive)
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

		let renderStart = -1;
		let visibleStart = -1;
		let visibleEnd = -1;
		let accumulatedHeight = 0;
		const scrollTop = viewport.scrollTop;
		const viewBottom = scrollTop + viewport.clientHeight;

		for (let i = 0; i < count; i++) {
			const rowHeight = visibleRowElements?.[i - renderRange.start]?.clientHeight;
			accumulatedHeight += rowHeight || getItemHeight(i);

			if (renderStart === -1 && accumulatedHeight >= scrollTop - renderDistance) {
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
		renderRange.start = startingAt;
		for (let i = startingAt; i < items.length; i++) {
			if (heightMap[i] !== undefined) {
				lockRowHeight(i);
			}
			renderRange.end = i + 1;
			await tick(); // Wait for element to be added
			const element = visibleRowElements?.[i - startingAt];
			if (!element) {
				// Can happen if a concurrent reactive update resets renderRange during tick().
				console.warn("[VList:init] element missing after tick, aborting");
				return;
			}
			heightMap[i] = element.clientHeight;
			if (
				calculateHeightSum(startingAt, renderRange.end) >
				viewport.clientHeight + renderDistance
			) {
				break;
			}
		}

		// Fill upwards if the viewport has remaining space.
		for (let i = startingAt - 1; i >= 0; i--) {
			if (heightMap[i] !== undefined) {
				lockRowHeight(i);
			}
			renderRange.start = i;
			await tick(); // Wait for element to be added
			const element = visibleRowElements?.[0];
			if (!element) {
				console.warn("[VList:init] element missing after tick, aborting");
				return;
			}
			const heightDiff = element.clientHeight - (lockedHeights[i] || defaultHeight);
			if (heightDiff !== 0) {
				viewport.scrollTop += heightDiff;
			}
			heightMap[i] = element.clientHeight;
			if (
				calculateHeightSum(renderRange.start, renderRange.end) >
				viewport.clientHeight + 2 * renderDistance
			) {
				break;
			}
		}

		if (!isInitialized()) {
			return;
		}

		updateOffsets();
		await tick();

		// Guard: viewport can become undefined during unmount due to reactive teardown.
		if (viewport) {
			viewport.scrollTop = calculateHeightSum(0, startingAt);
		}
	}

	async function recalculateRanges(): Promise<void> {
		if (!viewport || !visibleRowElements || !isInitialized() || isRecalculating) return;

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
			updateOffsets();

			// Lock heights for items entering the render range (below old range, then above)
			for (let i = start; i < Math.min(end, oldStart); i++) {
				if (heightMap[i]) lockRowHeight(i);
			}
			for (let i = Math.max(start, oldEnd); i < end; i++) {
				if (heightMap[i]) lockRowHeight(i);
			}

			await tick();
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
		if (!items || index < 0 || index > items.length - 1) {
			return;
		}
		lastScrollDirection = undefined;
		skipNextScrollEvent = true;
		lastJumpToIndex = index;
		lockRowHeight(index);
		initializeAt(index);
	}

	/**
	 * Updates heightMap when items change: resets on full replacement,
	 * shifts on prepend, or adjusts length for appends/removals.
	 */
	function updateHeightMap(countDelta: number, headChanged: boolean, tailChanged: boolean): void {
		if (headChanged && tailChanged && previousItems.length > 0) {
			// Items completely replaced (e.g., switching views) — clear stale
			// heights and renderRange so isInitialized() returns false below.
			heightMap = new Array(items.length);
			renderRange = { start: 0, end: 0 };
		} else if (headChanged && !tailChanged && countDelta > 0) {
			// Prepend: shift cached heights to match new indices.
			const shifted: number[] = new Array(items.length);
			for (let i = 0; i < previousItems.length; i++) {
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
			await tick();
			if (isNearBottom(previousDistance) || isNearBottom(distance)) {
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
			calculateHeightSum(0, renderRange.start) !== viewport.scrollTop &&
			renderRange.start === index
		) {
			// When scrolling up, compensate for the height change of the topmost
			// visible element to prevent content from jumping downward.
			viewport.scrollBy({ top: heightMap[index] - oldHeight });
		} else if ((lastJumpToIndex !== undefined || startIndex) && lastScrollDirection === undefined) {
			// After jumping to an index, maintain position as off-viewport elements
			// resize. Scroll direction is undefined during jumps.
			viewport.scrollTop = calculateHeightSum(0, lastJumpToIndex || startIndex || 0);
			skipNextScrollEvent = true;
		} else if (isNearBottom(previousDistance) && lastScrollDirection !== "up") {
			// Near bottom — snap back to bottom when:
			// 1. stickToBottom is on and we drifted >2px (the guard prevents a
			//    subpixel oscillation cascade from scrollToBottom bouncing by 1px)
			// 2. The very last item resized while we're not at scrollTop=0
			// Skip when scrolling up — the user is intentionally moving away from
			// the bottom, and snapping back creates a fight-the-user cascade.
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

	// When the viewport shrinks (e.g. on-screen keyboard opens), recalculate
	// and snap to bottom if we were already near it.
	$effect(() => {
		if (!viewportHeight) return;
		const shrunk = viewportHeight < previousViewportHeight;
		previousViewportHeight = viewportHeight;
		if (shrunk) {
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
		if (!items || items.length === 0) return;
		const countDelta = items.length - previousItems.length;
		const headChanged = !previousItems[0] || getId(items[0]) !== getId(previousItems[0]);
		const tailChanged =
			!previousItems.at(-1) || getId(items.at(-1)!) !== getId(previousItems.at(-1)!);

		untrack(() => handleItemsChanged(countDelta, headChanged, tailChanged));
		previousItems = [...items];
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
		if (skipNextScrollEvent) {
			skipNextScrollEvent = false;
			return;
		}
		updateScrollDirection();
		recalculateRanges();
	}}
	wide={grow}
	whenToShow={visibility}
	{padding}
>
	<div
		bind:this={container}
		data-remove-from-panning
		class="padded-contents"
		style:padding-top={offset.top + "px"}
		style:padding-bottom={offset.bottom + "px"}
	>
		{#each renderItems as data, i (renderRange.start + i)}
			<div
				class="list-row"
				data-index={i + renderRange.start}
				style:height={lockedHeights[i + renderRange.start]
					? `${lockedHeights[i + renderRange.start]}px`
					: undefined}
			>
				{@render template(data, renderRange.start + i)}
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
									initializeAt(items.length - 1);
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
		background-color: var(--clr-bg-1);
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
		border: 1px solid var(--clr-border-3);
		border-radius: var(--radius-btn);
		background-color: var(--clr-bg-2);
		color: var(--clr-text-1);
	}

	.feed-actions__scroll-to-bottom {
		z-index: var(--z-floating);
		overflow: hidden;
		border-radius: var(--radius-btn);
		background-color: var(--clr-bg-1);
		transition:
			box-shadow var(--transition-fast),
			transform var(--transition-medium);

		&:hover {
			transform: scale(1.05) translateY(-2px);
			box-shadow: var(--fx-shadow-s);
		}
	}
</style>
