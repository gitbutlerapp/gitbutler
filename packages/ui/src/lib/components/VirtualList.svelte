<!--
	VirtualList - A high-performance virtual scrolling component

	This component renders large lists efficiently by only rendering items that are
	currently visible in the viewport, making it ideal for displaying thousands of
	items without performance degradation.

	## Features
	- **Batched rendering**: Groups items into configurable chunks for optimal performance
	- **Dynamic height measurement**: Automatically measures and caches row heights
	- **Virtual positioning**: Uses padding offsets to maintain scroll position
	- **Infinite scrolling**: Triggers callback when nearing bottom for lazy loading
	- **Auto-layout updates**: Recalculates on resize and item changes
	- **Chat-like behavior**: Optional tail mode and sticky bottom scrolling
	- **User notifications**: Shows "new unread" and "scroll to bottom" buttons

	## Usage Example
	```svelte
	<VirtualList
		items={messages}
		batchSize={10}
		defaultHeight={50}
		stickToBottom={true}
		onloadmore={loadMoreMessages}
	>
		{#snippet chunkTemplate(chunk)}
			{#each chunk as message}
				<MessageRow {message} />
			{/each}
		{/snippet}
	</VirtualList>
	```

	## Performance Considerations
	- Set `batchSize` based on average item height and viewport size
	- Provide accurate `defaultHeight` to minimize layout shifts
	- Keep item rendering lightweight for smooth scrolling
	- Use the `onloadmore` callback for pagination with large datasets

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
		 * Snippet template for rendering a chunk of items.
		 * Receives an array of items to render as a batch.
		 */
		chunkTemplate: Snippet<[T[]]>;
		/**
		 * Number of items to group together in a single chunk.
		 * Larger values improve performance but may cause jumpier scrolling.
		 * Recommended: 5-20 items depending on item height.
		 */
		batchSize: number;
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
		 * Default height in pixels for each chunk before actual measurement.
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
	};

	const {
		items,
		children,
		chunkTemplate,
		batchSize,
		onloadmore,
		grow,
		padding,
		visibility,
		defaultHeight,
		stickToBottom = false
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

	// Debounce load more callback to prevent rapid consecutive triggers
	const debouncedLoadMore = $derived(debounce(() => onloadmore?.(), DEBOUNCE_DELAY));

	// DOM references
	/** The scrollable viewport element. */
	let viewport = $state<HTMLDivElement>();
	/** Live collection of rendered row elements. */
	let visibleRowElements = $state<HTMLCollectionOf<Element>>();
	/** Observes size changes of visible rows. */
	let itemObserver: ResizeObserver | null = null;
	/** Current height of the viewport. */
	let viewportHeight = $state(0);
	/** Previous viewport height to detect resize. */
	let previousViewportHeight = 0;

	// Virtual scrolling state
	/**
	 * Range of visible chunk indices.
	 * Initialized to Infinity when stickToBottom=true to defer calculation until DOM ready.
	 */
	let visibleRange = $state({
		start: stickToBottom ? Infinity : 0,
		end: stickToBottom ? Infinity : 0
	});
	/** Cache of measured heights for each chunk. */
	let heightMap: number[] = $state([]);
	/** Top and bottom padding to simulate off-screen content. */
	let offset = $state({ top: 0, bottom: 0 });
	/** Distance from bottom during last calculation (used for sticky scroll). */
	let previousDistance = $state(0);
	/** Whether initial range calculation has completed. */
	let isTailInitialized = $state(false);
	/** Previous item count to detect additions. */
	let previousCount = $state(items.length);
	/** Whether to show "new unread" notification. */
	let hasNewItemsAtBottom = $state(false);
	/** Prevents concurrent recalculation runs. */
	let isRecalculating = false;

	// Derived state
	/** Items divided into batches based on batchSize. */
	const itemChunks = $derived(divideIntoChunks(items, batchSize));
	/** Currently visible chunks with their data and IDs. */
	const visibleChunks = $derived.by(() =>
		itemChunks
			.slice(visibleRange.start, visibleRange.end)
			.map((data, i) => ({ id: i + visibleRange.start, data }))
	);

	// ============================================================================
	// Helper functions
	// ============================================================================

	function divideIntoChunks<T>(array: T[], size: number): T[][] {
		return Array.from({ length: Math.ceil(array.length / size) }, (_v, i) =>
			array.slice(i * size, size * (i + 1))
		);
	}

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

	function saveDistanceFromBottom(): void {
		if (viewport) {
			previousDistance = getDistanceFromBottom();
		}
	}

	function shouldTriggerLoadMore(): boolean {
		if (!viewport) return false;
		if (viewport.scrollHeight <= viewport.clientHeight) return true;
		const distance = getDistanceFromBottom();
		return distance > 0 && distance < LOAD_MORE_THRESHOLD;
	}

	function wasNearBottom(): boolean {
		return previousDistance < STICKY_DISTANCE;
	}

	// ============================================================================
	// Range calculation functions
	// ============================================================================

	/**
	 * Calculates which chunk should be at the top of the visible range.
	 * Iterates from top until accumulated height exceeds scroll position.
	 * @returns Index of first visible chunk
	 */
	function calculateVisibleStartIndex(): number {
		if (itemChunks.length === 0 || !viewport) return 0;

		let accumulatedHeight = 0;
		for (let i = 0; i < itemChunks.length; i++) {
			const rowHeight = visibleRowElements?.[i - visibleRange.start]?.clientHeight;
			accumulatedHeight += rowHeight || heightMap[i] || defaultHeight;
			if (accumulatedHeight > viewport.scrollTop) {
				return i;
			}
		}
		return itemChunks.length - 1;
	}

	/**
	 * Calculates which chunk should be at the bottom of the visible range.
	 * Iterates from visible start until accumulated height exceeds viewport height.
	 * @returns Index after last visible chunk (exclusive)
	 */
	function calculateVisibleEndIndex(): number {
		if (!viewport) return itemChunks.length;

		let accumulatedHeight = offset.top - viewport.scrollTop;
		for (let i = visibleRange.start; i < itemChunks.length; i++) {
			accumulatedHeight += heightMap[i] || defaultHeight;
			if (accumulatedHeight > viewport.clientHeight) {
				return i + 1;
			}
		}
		return itemChunks.length;
	}

	/**
	 * Calculates visible start index when initializing from bottom (stickToBottom).
	 * Works backwards from the end to fill viewport height.
	 * @returns Index of first chunk that should be visible
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
	 * Initializes visible range in tail mode.
	 * Sets range to show last items and scrolls to bottom.
	 */
	async function initialize() {
		if (!viewport) return;
		visibleRange.start = 0;

		const end = calculateVisibleEndIndex();
		for (let i = 0; i < end; i++) {
			visibleRange.end = i + 1;
			await tick();
			const element = visibleRowElements?.item(i);
			if (element?.clientHeight) {
				heightMap[i] = element.clientHeight;
			}
			if (calculateHeightSum(0, i + 1) > viewport.clientHeight) {
				break;
			}
		}
		offset = { top: 0, bottom: calculateHeightSum(visibleRange.end, itemChunks.length) };
		isTailInitialized = true;
	}

	/**
	 * Initializes visible range in tail mode.
	 * Sets range to show last items and scrolls to bottom.
	 */
	async function initializeTail() {
		if (!viewport) return;
		const end = itemChunks.length;
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
		isTailInitialized = true;
	}

	/**
	 * Updates the visible range based on current scroll position.
	 * Recalculates which chunks should be rendered and updates offsets.
	 */
	function updateRange(): void {
		if (!viewport || !visibleRowElements) return;
		visibleRange = {
			start: calculateVisibleStartIndex(),
			end: calculateVisibleEndIndex()
		};
		offset = {
			bottom: calculateHeightSum(visibleRange.end, heightMap.length),
			top: calculateHeightSum(0, visibleRange.start)
		};
	}

	/**
	 * Main layout calculation function.
	 * Determines which chunks should be visible, sets up resize observers,
	 * and triggers onloadmore if needed. Called on scroll and item changes.
	 */
	async function recalculateVisibleRange(): Promise<void> {
		if (!viewport || !visibleRowElements || isRecalculating) return;

		isRecalculating = true;
		heightMap.length = itemChunks.length;

		const distance = getDistanceFromBottom();
		let scrollTop = viewport.scrollTop;

		// First-time initialization for tail mode
		if (!isTailInitialized) {
			if (stickToBottom) {
				await initializeTail();
				scrollTop = viewport!.scrollHeight;
				scrollToBottom();
			} else {
				await initialize();
			}
		} else {
			updateRange();
		}

		// Content, sizes, and scroll are affected by this tick.
		await tick();

		if (stickToBottom && distance === 0 && distance !== getDistanceFromBottom()) {
			// A difference in viewport scrollHeight is occasionally observerd
			// before and after the `await tick()` call just above. It could
			// be the result of an incorrect height/offset calculation, or it
			// just content shifting in place after render. If that happens for
			// we can lose touch with the bottom.
			scrollToBottom();
		} else {
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

		for (const rowElement of visibleRowElements) {
			// It seems unnecessary to track duplicates and removals, so
			// not doing it to keep things concise.
			itemObserver?.observe(rowElement);
		}

		// Saved distance is necessary when sticking to bottom.
		saveDistanceFromBottom();
		isRecalculating = false;
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
		itemObserver = new ResizeObserver((entries) =>
			untrack(() => {
				for (const entry of entries) {
					const { target } = entry;
					if (!target.isConnected) continue;

					const indexStr = target.getAttribute('data-index');
					const index = indexStr ? parseInt(indexStr, 10) : undefined;
					if (index !== undefined && !(index in heightMap)) {
						heightMap[index] = target.clientHeight;
						// In tail mode, adjust scroll when top item's height is measured
						if (stickToBottom && index === visibleRange.start) {
							const heightDiff = target.clientHeight - defaultHeight;
							if (heightDiff !== 0 && viewport && target.clientTop <= viewport.scrollTop) {
								viewport.scrollBy({ top: heightDiff });
							}
						}
						// Scroll to bottom when last item resizes during initialization or sticky mode
						if (stickToBottom && index === visibleRange.end - 1) {
							scrollToBottom();
						}
					}
				}

				// Auto-scroll if content grew while user is near bottom
				if (viewport && stickToBottom && wasNearBottom()) {
					const hasGrown = getDistanceFromBottom() > previousDistance;
					if (hasGrown) {
						scrollToBottom();
					}
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
					recalculateVisibleRange();
				}
			});
		}
		previousCount = items.length;
	});

	/**
	 * Clears "new items" indicator when user scrolls to see all items.
	 */
	$effect(() => {
		if (hasNewItemsAtBottom && visibleRange.end === itemChunks.length) {
			hasNewItemsAtBottom = false;
		}
	});

	export function scrollToBottom(): void {
		if (!viewport) return;
		viewport.scrollTop = viewport.scrollHeight - viewport.clientHeight;
	}
</script>

<ScrollableContainer
	bind:viewportHeight
	bind:viewport
	onscroll={recalculateVisibleRange}
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
		{#each visibleChunks as chunk, i (chunk.id)}
			<div class="list-row" data-index={i + visibleRange.start}>
				{@render chunkTemplate(chunk.data)}
			</div>
		{/each}
		<div
			class="children"
			use:resizeObserver={({ frame: { height } }) => {
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
				onclick={scrollToBottom}
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
		overflow: hidden;
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
