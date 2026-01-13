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
	import Button from '$components/Button.svelte';
	import ScrollableContainer from '$components/scroll/ScrollableContainer.svelte';

	import { debounce } from '$lib/utils/debounce';

	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { tick, untrack, type Snippet } from 'svelte';
	import { fade } from 'svelte/transition';
	import type { ScrollbarVisilitySettings } from '$components/scroll/Scrollbar.svelte';

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
		 * Async callback triggered when scroll approaches the bottom.
		 * Fires when within 200px of bottom or when content is shorter than viewport.
		 * Useful for implementing infinite scrolling/pagination.
		 */
		onloadmore?: () => Promise<void>;
		/**
		 * Whether the list should grow to fill available space.
		 * Passed to the underlying ScrollableContainer.
		 */
		grow?: boolean;
		/**
		 * Initialize scroll position at bottom instead of top.
		 * Auto-scroll to bottom when new items are added and user is near bottom.
		 * Only triggers if within 40px of bottom. Ideal for live feeds and chat interfaces.
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
	};

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
		startIndex
	}: Props = $props();

	// Tuning constants
	/** Distance from bottom (px) before showing "scroll to bottom" button. */
	const SCROLL_DOWN_THRESHOLD = 600;
	/** Distance from bottom (px) considered "near bottom" for sticky scroll. */
	const STICKY_DISTANCE = 40;
	/** Distance from bottom (px) before triggering onloadmore callback. */
	const LOAD_MORE_THRESHOLD = 200;
	/** Debounce delay (ms) for onloadmore to prevent rapid firing. */
	const DEBOUNCE_DELAY = 50;
	/** Duration (ms) to lock row height when it comes into view. */
	const HEIGHT_LOCK_DURATION = 1000;

	// Debounce load more callback to prevent rapid consecutive triggers
	const debouncedLoadMore = $derived(debounce(() => onloadmore?.(), DEBOUNCE_DELAY));

	// DOM references
	/** The scrollable viewport element. */
	let viewport = $state<HTMLDivElement>();
	/** Live collection of rendered row elements. */
	let visibleRowElements = $state<HTMLCollectionOf<Element>>();
	/** Observes size changes of visible rows. */
	let itemObserver: ResizeObserver | null = null;
	/** Array of element references for observed items. */
	let observedElements: (Element | undefined)[] = [];
	/** Cache of measured heights for each item. */
	let heightMap: number[] = $state([]);
	/** Array of locked heights for items (prevents layout shift during async loads). */
	let lockedHeights = $state<number[]>([]);
	/** Array of unlock timeouts for items. */
	let heightUnlockTimeouts: number[] = [];
	/** Current height of the viewport. */
	let viewportHeight = $state(0);
	/** Previous viewport height to detect resize. */
	let previousViewportHeight = 0;

	// Virtual scrolling state
	/**
	 * Range of visible item indices.
	 * Initialized to Infinity when stickToBottom=true to defer calculation until DOM ready.
	 */
	let visibleRange = $state({
		start: stickToBottom ? Infinity : startIndex || 0,
		end: stickToBottom ? Infinity : startIndex ? startIndex + 1 : 0
	});

	/** Top and bottom padding to simulate off-screen content. */
	let offset = $state({ top: 0, bottom: 0 });
	/** Distance from bottom during last calculation (used for sticky scroll). */
	let previousDistance = $state(0);
	/** Whether initial range calculation has completed. */
	let isInitialized = $state(false);
	/** Previous item count to detect additions. */
	let previousCount = $state(items.length);
	/** Whether to show "new unread" notification. */
	let hasNewItemsAtBottom = $state(false);
	/** Prevents concurrent recalculation runs. */
	let isRecalculating = false;

	/** Used to determine the direction of the most recent scroll. */
	let lastScrollTop: number | undefined = undefined;
	/** Used for understanding if elements resizing should scroll to offset content shift. */
	let lastScrollDirection: 'up' | 'down' | undefined;
	/** Last index that was scrolled to with `jumpToIndex`. */
	let lastJumpToIndex: number | undefined;
	/** Used to skip the next onscroll event after programmatic scrolling. */
	let ignoreScroll = false;
	/** Currently visible items with their data and IDs. */
	const visible = $derived.by(() =>
		items
			.slice(visibleRange.start, visibleRange.end)
			.map((data, i) => ({ id: i + visibleRange.start, data }))
	);

	// ============================================================================
	// Helper functions
	// ============================================================================

	function calculateHeightSum(startIndex: number, endIndex: number): number {
		let sum = 0;
		for (let i = startIndex; i < endIndex; i++) {
			sum += heightMap[i] || defaultHeight;
		}
		return sum;
	}

	function getDistanceFromBottom(): number {
		if (!viewport) return 0;
		return viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
	}

	function shouldTriggerLoadMore(): boolean {
		if (!viewport) return false;
		if (viewport.scrollHeight <= viewport.clientHeight) return true;
		const distance = getDistanceFromBottom();
		return distance >= 0 && distance < LOAD_MORE_THRESHOLD;
	}

	function wasNearBottom(): boolean {
		return previousDistance < STICKY_DISTANCE;
	}

	/**
	 * Locks a row height based on cached value and schedules unlock.
	 * This prevents layout shifts when async content loads.
	 */
	function lockRowHeight(index: number): void {
		const cachedHeight = heightMap[index];
		if (!cachedHeight) return;

		lockedHeights[index] = cachedHeight;

		// Clear any existing timeout for this index
		const existingTimeout = heightUnlockTimeouts[index];
		if (existingTimeout) {
			clearTimeout(existingTimeout);
		}

		// Schedule removal of locked height
		const timeoutId = window.setTimeout(() => {
			delete lockedHeights[index];
			delete heightUnlockTimeouts[index];
		}, HEIGHT_LOCK_DURATION);

		heightUnlockTimeouts[index] = timeoutId;
	}

	// ============================================================================
	// Range calculation functions
	// ============================================================================

	/**
	 * Calculates which item should be at the top of the visible range.
	 * Iterates from top until accumulated height exceeds scroll position.
	 * @returns Index of first visible item
	 */
	function calculateVisibleStartIndex(): number {
		if (items.length === 0 || !viewport) return 0;

		let accumulatedHeight = 0;
		for (let i = 0; i < items.length; i++) {
			const rowHeight = visibleRowElements?.[i - visibleRange.start]?.clientHeight;
			const heightToUse = rowHeight || heightMap[i] || defaultHeight;
			accumulatedHeight += heightToUse;
			if (accumulatedHeight > viewport.scrollTop) {
				return i;
			}
		}
		return items.length - 1;
	}

	/**
	 * Calculates which item should be at the bottom of the visible range.
	 * Iterates from visible start until accumulated height exceeds viewport height.
	 * @returns Index after last visible item (exclusive)
	 */
	function calculateVisibleEndIndex(): number {
		if (!viewport) return items.length;

		let accumulatedHeight = offset.top - viewport.scrollTop;
		for (let i = visibleRange.start; i < items.length; i++) {
			accumulatedHeight += heightMap[i] || defaultHeight;
			if (accumulatedHeight > viewport.clientHeight) {
				return i + 1;
			}
		}
		return items.length;
	}

	/**
	 * Calculates visible start index when initializing from bottom (stickToBottom).
	 * Works backwards from the end to fill viewport height.
	 * @returns Index of first item that should be visible
	 */
	function calculateStartIndexFromBottom(): number {
		if (!viewport) return 0;

		let accumulated = 0;
		for (let i = visibleRange.end - 1; i >= 0; i--) {
			accumulated += heightMap[i] || defaultHeight;
			if (accumulated > viewport.clientHeight) {
				return i;
			}
		}
		return 0;
	}

	/**
	 * Initializes visible range from the top.
	 * Measures items starting from the first item until viewport is filled.
	 */
	async function initializeAt(startingAt: number) {
		if (!viewport) return;

		visibleRange.start = startingAt;
		for (let i = startingAt; i < items.length; i++) {
			visibleRange.end = i + 1;
			await tick();
			const element = visibleRowElements?.item(i - startingAt);
			if (element) {
				heightMap[i] = element.clientHeight;
			}
			if (calculateHeightSum(startingAt, visibleRange.end) > viewport.clientHeight) {
				break;
			}
		}

		if (calculateHeightSum(visibleRange.start, visibleRange.end) < viewport.clientHeight) {
			for (let i = visibleRange.start - 1; i >= 0; i--) {
				visibleRange.start = i;
				await tick();
				const element = visibleRowElements?.item(0);
				if (element) {
					heightMap[i] = element.clientHeight;
				}
				const height = calculateHeightSum(visibleRange.start, visibleRange.end);
				if (height > viewport.clientHeight) {
					break;
				}
			}
		}

		updateOffsets();
		await tick();

		viewport.scrollTop = calculateHeightSum(0, startingAt);
		updateElementObservers();
	}

	/**
	 * Initializes visible range in tail mode.
	 * Sets range to show last items and scrolls to bottom.
	 */
	async function initializeTail() {
		if (!viewport) return;
		const end = items.length;
		visibleRange.end = end;
		offset.bottom = 0;

		const start = calculateStartIndexFromBottom();
		for (let i = end; i >= start; i--) {
			visibleRange.start = i - 1;
			await tick();
			const element = visibleRowElements?.item(0);
			if (element?.clientHeight) {
				heightMap[i - 1] = element.clientHeight;
			}
			if (calculateHeightSum(i - 1, end) > viewport.clientHeight) {
				break;
			}
		}
		offset.top = calculateHeightSum(0, visibleRange.start);
		isInitialized = true;
	}

	/**
	 * Calculates which item indices are newly visible after a range change.
	 * Compares old and new ranges to find items entering the viewport.
	 */
	function getNewlyVisibleIndices(
		oldStart: number,
		oldEnd: number,
		newStart: number,
		newEnd: number
	): number[] {
		const newIndices: number[] = [];

		// New items at the start (scrolling up)
		for (let i = newStart; i < Math.min(oldStart, newEnd); i++) {
			newIndices.push(i);
		}

		// New items at the end (scrolling down)
		for (let i = Math.max(oldEnd, newStart); i < newEnd; i++) {
			newIndices.push(i);
		}

		return newIndices;
	}

	/**
	 * Main layout calculation function.
	 * Determines which items should be visible, sets up resize observers,
	 * and triggers onloadmore if needed. Called on scroll and item changes.
	 */
	async function recalculateVisibleRange(): Promise<void> {
		if (!viewport || !visibleRowElements || isRecalculating) return;

		isRecalculating = true;
		heightMap.length = items.length;

		const distance = getDistanceFromBottom();
		let scrollTop = viewport.scrollTop;

		let wasInitialized = isInitialized;

		// First-time initialization for tail mode
		if (!isInitialized) {
			if (stickToBottom) {
				await initializeTail();
				scrollTop = viewport.scrollHeight;
				scrollToBottom();
			} else {
				await initializeAt(startIndex || 0);
				isInitialized = true;
			}
			updateOffsets();
		} else {
			// Capture old range before updating
			const oldStart = visibleRange.start;
			const oldEnd = visibleRange.end;

			// Update visible range based on scroll position
			if (!viewport || !visibleRowElements) return;
			visibleRange = {
				start: calculateVisibleStartIndex(),
				end: calculateVisibleEndIndex()
			};
			updateOffsets();

			// Find and lock heights for items entering viewport
			const newIndices = getNewlyVisibleIndices(
				oldStart,
				oldEnd,
				visibleRange.start,
				visibleRange.end
			);
			for (const index of newIndices) {
				if (heightMap[index]) {
					lockRowHeight(index);
				}
			}
		}

		// Content, sizes, and scroll are affected by this tick.
		await tick();

		if (stickToBottom && distance === 0 && distance !== getDistanceFromBottom()) {
			// A difference in viewport scrollHeight is occasionally observed
			// before and after the `await tick()` call just above. It could
			// be the result of an incorrect height/offset calculation, or it
			// could be content shifting in place after render. If that happens,
			// we can lose touch with the bottom.
			scrollToBottom();
		} else if (wasInitialized) {
			if (viewport.scrollTop !== scrollTop) {
				// Strange scroll motion appears out of thin air, so we reset
				// it here to keep things smooth. It could have something to do
				// with content shrink is offset by padding. Would be great
				// to understand it better.
				viewport.scrollTop = scrollTop;
			}
		}

		if (shouldTriggerLoadMore()) {
			debouncedLoadMore();
		}

		updateElementObservers();

		// Saved distance is necessary when sticking to bottom.
		previousDistance = getDistanceFromBottom();
		isRecalculating = false;
	}

	function updateElementObservers() {
		if (!viewport || !visibleRowElements) return;
		// Unobserve elements that are no longer in the visible range
		for (let i = 0; i < observedElements.length; i++) {
			const element = observedElements[i];
			if (element && (i < visibleRange.start || i >= visibleRange.end)) {
				itemObserver?.unobserve(element);
				observedElements[i] = undefined;
			}
		}

		// Observe new visible elements
		for (const rowElement of visibleRowElements) {
			const indexStr = rowElement.getAttribute('data-index');
			const index = indexStr ? parseInt(indexStr, 10) : undefined;

			if (index !== undefined && !observedElements[index]) {
				itemObserver?.observe(rowElement);
				observedElements[index] = rowElement;
			}
		}
	}

	function updateOffsets() {
		offset = {
			top: calculateHeightSum(0, visibleRange.start),
			bottom: calculateHeightSum(visibleRange.end, heightMap.length)
		};
	}

	// ============================================================================
	// Reactive effects
	// ============================================================================

	/**
	 * Sets up ResizeObserver to track size changes of visible rows.
	 * Updates heightMap when rows are measured and handles scroll adjustments.
	 */
	$effect(() => {
		if (!viewport) return;

		visibleRowElements = viewport.getElementsByClassName('list-row');

		// Clean up previous observer if it exists
		if (itemObserver) {
			itemObserver.disconnect();
			observedElements = [];

			// Clear all pending height unlock timeouts
			for (const timeoutId of heightUnlockTimeouts) {
				if (timeoutId) clearTimeout(timeoutId);
			}
			heightUnlockTimeouts = [];
			lockedHeights = [];
		}

		itemObserver = new ResizeObserver((entries) =>
			untrack(() => {
				if (!viewport) return;
				let shouldRecalculate = false;
				for (const entry of entries) {
					const { target } = entry;
					if (!target.isConnected) continue;

					const indexStr = target.getAttribute('data-index');
					const index = indexStr ? parseInt(indexStr, 10) : undefined;
					if (index !== undefined) {
						const firstRender = !(index in heightMap);
						const oldHeight = heightMap[index] || defaultHeight;

						if (heightMap[index] !== target.clientHeight) {
							heightMap[index] = target.clientHeight;
						}

						if (firstRender) {
							// Scroll to bottom when last item resizes during initialization or sticky mode
							if (stickToBottom && index === visibleRange.end - 1) {
								scrollToBottom();
							}
						} else if (
							lastScrollDirection === 'up' &&
							calculateHeightSum(0, visibleRange.start) !== viewport.scrollTop
						) {
							viewport?.scrollBy({ top: heightMap[index] - oldHeight });
						} else if (index < visibleRange.end - 1 && lastScrollDirection === 'down') {
							viewport?.scrollBy({ top: heightMap[index] - oldHeight });
						} else if (lastJumpToIndex !== undefined && lastScrollDirection === undefined) {
							// After jumpToIndex, maintain position as items measure themselves
							viewport?.scrollTo({ top: calculateHeightSum(0, lastJumpToIndex) });
							ignoreScroll = true;
						} else if (stickToBottom && wasNearBottom()) {
							ignoreScroll = true;
							scrollToBottom();
						}
						shouldRecalculate = true;
					}
				}

				if (shouldRecalculate) {
					recalculateVisibleRange();
				}
			})
		);

		return () => itemObserver?.disconnect();
	});

	/**
	 * Recalculates layout when viewport height changes (e.g., window resize).
	 * Maintains sticky bottom behavior if enabled.
	 */
	$effect(() => {
		if (viewportHeight && previousViewportHeight !== viewportHeight) {
			untrack(async () => {
				await recalculateVisibleRange();
				if (stickToBottom && wasNearBottom()) {
					scrollToBottom();
				}
				previousViewportHeight = viewportHeight;
			});
		}
	});

	/**
	 * Handles new items being added to the list.
	 * Auto-scrolls if sticky bottom is enabled, otherwise shows notification.
	 */
	$effect(() => {
		if (items) {
			untrack(async () => {
				if (stickToBottom && wasNearBottom()) {
					// User is at the bottom, auto-scroll
					await recalculateVisibleRange();
					scrollToBottom();
				} else {
					// Check if there are new items at the bottom
					if (stickToBottom) {
						const count = items.length;
						hasNewItemsAtBottom = count > previousCount && count > visibleRange.end;
					}
				}
			});
		}
		previousCount = items.length;
	});

	/**
	 * Clears "new items" indicator when user scrolls to see all items.
	 */
	$effect(() => {
		if (hasNewItemsAtBottom && visibleRange.end === items.length) {
			hasNewItemsAtBottom = false;
		}
	});

	export function scrollToBottom(): void {
		if (!viewport) return;
		viewport.scrollTop = viewport.scrollHeight - viewport.clientHeight;
	}

	/**
	 * Scrolls to a specific item index in the list by reinitializing the list.
	 */
	export async function jumpToIndex(index: number) {
		if (index < 0 || index > items.length - 1) {
			return;
		}
		unobserveAll();
		lastScrollDirection = undefined;
		initializeAt(index);
		lastJumpToIndex = index;
	}

	function unobserveAll() {
		for (let i = 0; i < observedElements.length; i++) {
			const element = observedElements[i];
			if (element) {
				itemObserver?.unobserve(element);
				observedElements[i] = undefined;
			}
		}
	}
</script>

<ScrollableContainer
	bind:viewportHeight
	bind:viewport
	onscroll={() => {
		if (!viewport) return;
		if (ignoreScroll) {
			ignoreScroll = false;
			return;
		}
		const scrollTop = viewport?.scrollTop;
		if (lastScrollTop && lastScrollTop > scrollTop) {
			lastScrollDirection = 'up';
		} else if (lastScrollTop && lastScrollTop < scrollTop) {
			lastScrollDirection = 'down';
		} else {
			lastScrollDirection = undefined;
		}
		recalculateVisibleRange();
		lastScrollTop = viewport.scrollTop;
	}}
	wide={grow}
	whenToShow={visibility}
	{padding}
>
	<div
		data-remove-from-panning
		class="padded-contents"
		style:padding-top={offset.top + 'px'}
		style:padding-bottom={offset.bottom + 'px'}
	>
		{#each visible as item, i (item.id)}
			<div
				class="list-row"
				data-index={i + visibleRange.start}
				style:height={lockedHeights[i + visibleRange.start]
					? `${lockedHeights[i + visibleRange.start]}px`
					: undefined}
			>
				{@render template(item.data, visibleRange.start + i)}
			</div>
		{/each}
		<div
			class="children"
			use:resizeObserver={({ frame: { height } }) => {
				if (!stickToBottom) return;
				const distance = getDistanceFromBottom();
				if (wasNearBottom()) {
					scrollToBottom();
				} else if (distance <= height * 1.2) {
					// It is sometimes the case that `previousDistance` has already
					// updated to include children height, and therefore `wasNearBottom`
					// is false. Also, the value seems to be off by one, but we can
					// generously give this condition a bit of margin.
					scrollToBottom();
				}
			}}
		>
			{@render children?.()}
		</div>
	</div>
</ScrollableContainer>
{#if previousDistance > SCROLL_DOWN_THRESHOLD || hasNewItemsAtBottom}
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
		{#if previousDistance > SCROLL_DOWN_THRESHOLD}
			<div class="feed-actions__scroll-to-bottom" transition:fade={{ duration: 150 }}>
				<Button
					kind="outline"
					icon="arrow-down"
					tooltip="Scroll to bottom"
					onclick={scrollToBottom}
				/>
			</div>
		{/if}
	</div>
{/if}

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
