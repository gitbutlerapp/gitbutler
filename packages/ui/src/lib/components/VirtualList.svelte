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

	// ============================================================================
	// Constants
	// ============================================================================

	/** Distance from bottom (px) before showing "scroll to bottom" button. */
	const SCROLL_DOWN_THRESHOLD = 600;
	/** Distance from bottom (px) considered "near bottom" for sticky scroll. */
	const STICKY_DISTANCE = 40;
	/** Distance from bottom (px) before triggering onloadmore callback. */
	const LOAD_MORE_THRESHOLD = 300;
	/** Debounce delay (ms) for onloadmore to prevent rapid firing. */
	const DEBOUNCE_DELAY = 50;
	/** Duration (ms) to lock row height when it comes into view. */
	const HEIGHT_LOCK_DURATION = 1000;

	// ============================================================================
	// Props
	// ============================================================================

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

	// ============================================================================
	// DOM References
	// ============================================================================

	/** The scrollable viewport element. */
	let viewport = $state<HTMLDivElement>();
	/** Container element for list items. */
	let container = $state<HTMLDivElement>();
	/** Live collection of rendered row elements. */
	let visibleRowElements = $state<HTMLCollectionOf<Element>>();

	// ============================================================================
	// State - Height tracking
	// ============================================================================

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

	// ============================================================================
	// State - Virtual scrolling
	// ============================================================================

	/**
	 * Range of visible item indices.
	 * Initialized to Infinity when stickToBottom=true to defer calculation until DOM ready.
	 */
	let visibleRange = $state({
		start: 0,
		end: 0
	});
	/** Top and bottom padding to simulate off-screen content. */
	let offset = $state({ top: 0, bottom: 0 });
	/** Distance from bottom during last calculation (used for sticky scroll). */
	let previousDistance = $state(0);
	/** Previous item count to detect additions. */
	let previousCount = $state(items.length);
	/** Whether to show "new unread" notification. */
	let hasNewItemsAtBottom = $state(false);
	/** Prevents concurrent recalculation runs. */
	let isRecalculating = false;

	// ============================================================================
	// State - Scroll tracking
	// ============================================================================

	/** Used to determine the direction of the most recent scroll. */
	let lastScrollTop: number | undefined = undefined;
	/** Used for understanding if elements resizing should scroll to offset content shift. */
	let lastScrollDirection: 'up' | 'down' | undefined;
	/** Last index that was scrolled to with `jumpToIndex`. */
	let lastJumpToIndex: number | undefined;
	/** Used to skip the next onscroll event after programmatic scrolling. */
	let ignoreScroll = false;

	// ============================================================================
	// Derived values
	// ============================================================================

	/** Debounce load more callback to prevent rapid consecutive triggers */
	const debouncedLoadMore = $derived(debounce(() => onloadmore?.(), DEBOUNCE_DELAY));

	/** Currently visible items with their data and IDs. */
	const visible = $derived.by(() =>
		items
			.slice(visibleRange.start, visibleRange.end)
			.map((data, i) => ({ id: i + visibleRange.start, data }))
	);

	// ============================================================================
	// Helper functions
	// ============================================================================

	/** Whether initial range calculation has completed. */
	function isInitialized() {
		return visibleRange.end !== 0;
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
	 * Initializes visible range from the top.
	 */
	async function initializeAt(startingAt: number): Promise<boolean> {
		if (!viewport) return false;

		visibleRange.start = startingAt;
		for (let i = startingAt; i < items.length; i++) {
			visibleRange.end = i + 1;
			await tick();
			const element = visibleRowElements?.[i - startingAt];
			if (!element) {
				throw new Error('Expected to find element in loop 1.');
			}
			heightMap[i] = element.clientHeight;
			const height = calculateHeightSum(startingAt, visibleRange.end);
			log('loop 1', i, height);
			if (height > viewport.clientHeight) {
				break;
			}
		}

		if (calculateHeightSum(visibleRange.start, visibleRange.end) < viewport.clientHeight) {
			for (let i = visibleRange.start - 1; i >= 0; i--) {
				visibleRange.start = i;
				await tick();
				const element = visibleRowElements?.[0];
				if (!element) {
					throw new Error('Expected to find element in loop2.');
				}
				heightMap[i] = element.clientHeight;
				const height = calculateHeightSum(visibleRange.start, visibleRange.end);
				log('loop 2', i, height);
				if (height > viewport.clientHeight) {
					break;
				}
			}
		}

		if (!isInitialized()) {
			return false;
		}

		updateOffsets();
		await tick();
		const scrollTop = calculateHeightSum(0, startingAt);
		viewport.scrollTop = scrollTop;
		log('initialized at', scrollTop, viewport.scrollTop);

		return true;
	}

	/**
	 * Main layout calculation function.
	 * Determines which items should be visible, sets up resize observers,
	 * and triggers onloadmore if needed. Called on scroll and item changes.
	 */
	async function recalculateVisibleRange(): Promise<void> {
		if (!viewport || !visibleRowElements || !isInitialized() || isRecalculating) return;
		log('recalculating visible range', viewport.offsetTop, viewport.offsetHeight);

		isRecalculating = true;

		// Capture old range before updating
		const oldStart = visibleRange.start;
		const oldEnd = visibleRange.end;
		const bottomDistance = getDistanceFromBottom();

		// Update visible range based on scroll position
		if (!viewport || !visibleRowElements) return;
		visibleRange = {
			start: calculateVisibleStartIndex(),
			end: calculateVisibleEndIndex()
		};
		updateOffsets();

		// Find and lock heights for items entering viewport
		const { newMinusOld } = subtractRanges(oldStart, oldEnd, visibleRange.start, visibleRange.end);
		for (const index of newMinusOld) {
			if (heightMap[index]) {
				lockRowHeight(index);
			}
		}

		// Content, sizes, and scroll are affected by this tick.
		await tick();

		if (stickToBottom && bottomDistance === 0 && bottomDistance !== getDistanceFromBottom()) {
			log('path 11');
			scrollToBottom();
		}

		// Saved distance is necessary when sticking to bottom.
		previousDistance = getDistanceFromBottom();
		isRecalculating = false;

		if (shouldTriggerLoadMore()) {
			debouncedLoadMore();
		}
	}

	function updateOffsets() {
		offset = {
			top: calculateHeightSum(0, visibleRange.start),
			bottom: calculateHeightSum(visibleRange.end, heightMap.length)
		};
	}

	function subtractRanges(oldStart: number, oldEnd: number, newStart: number, newEnd: number) {
		const oldMinusNew: number[] = [];
		const newMinusOld: number[] = [];

		// Helper to push integer ranges into arrays
		function pushRange(arr: number[], start: number, end: number) {
			for (let i = start; i < end; i++) arr.push(i);
		}

		// old minus new
		if (oldStart < newStart) {
			pushRange(oldMinusNew, oldStart, Math.min(oldEnd, newStart));
		}
		if (oldEnd > newEnd) {
			pushRange(oldMinusNew, Math.max(oldStart, newEnd), oldEnd);
		}

		// new minus old
		if (newStart < oldStart) {
			pushRange(newMinusOld, newStart, Math.min(newEnd, oldStart));
		}
		if (newEnd > oldEnd) {
			pushRange(newMinusOld, Math.max(newStart, oldEnd), newEnd);
		}

		return { oldMinusNew, newMinusOld };
	}

	// ============================================================================
	// Public API
	// ============================================================================

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
		lastScrollDirection = undefined;
		lastJumpToIndex = index;
		initializeAt(index);
	}

	// ============================================================================
	// Observers
	// ============================================================================

	const itemObserver = new ResizeObserver((entries) => {
		if (!viewport) return;
		let shouldRecalculate = false;
		for (const entry of entries) {
			const { target } = entry;
			if (!target.isConnected) continue;
			if (!(target instanceof HTMLElement)) continue;

			// Resize events fire for disconnected elements, so we use this boolean
			// to selectively recalculate visible range at the end.
			shouldRecalculate = true;

			const indexAttr = target.dataset.index;
			const index = indexAttr ? parseInt(indexAttr) : undefined;
			if (index !== undefined) {
				const oldHeight = heightMap[index] || defaultHeight;
				if (heightMap[index] !== target.clientHeight) {
					heightMap[index] = target.clientHeight;
				}

				if (
					lastScrollDirection === 'up' &&
					calculateHeightSum(0, visibleRange.start) !== viewport.scrollTop
				) {
					log('path 1');
					viewport.scrollBy({ top: heightMap[index] - oldHeight });
				} else if (
					(lastJumpToIndex !== undefined || startIndex) &&
					lastScrollDirection === undefined
				) {
					log('path 2');
					viewport.scrollTop = calculateHeightSum(0, lastJumpToIndex || startIndex || 0);
					ignoreScroll = true;
				} else if (stickToBottom && wasNearBottom()) {
					log('path 3');
					ignoreScroll = true;
					scrollToBottom();
				}
			}
		}

		if (shouldRecalculate) {
			recalculateVisibleRange();
		}
	});

	// Use MutationObserver to detect new direct children
	const mo = new MutationObserver((mutations) => {
		for (const mutation of mutations) {
			for (const node of mutation.addedNodes) {
				if (node instanceof HTMLElement && node.matches('[data-index]')) {
					itemObserver.observe(node);
				}
			}
			// Optional: handle removed nodes to disconnect observers
			for (const node of mutation.removedNodes) {
				if (node instanceof HTMLElement) {
					itemObserver.unobserve(node);
				}
			}
		}
	});

	// ============================================================================
	// Effects
	// ============================================================================

	/**
	 * Observe all existing direct children
	 */
	$effect(() => {
		if (container) {
			mo.observe(container, { childList: true });
			for (const el of container.querySelectorAll(':scope > [data-index]')) {
				itemObserver.observe(el);
			}
			return () => mo.disconnect();
		}
	});

	/**
	 * Disconnect the item observer on umount.
	 */
	$effect(() => {
		return () => itemObserver.disconnect();
	});

	/**
	 * Sets up collection of visible row elements.
	 */
	$effect(() => {
		if (!viewport) return;
		visibleRowElements = viewport.getElementsByClassName('list-row');
	});

	/**
	 * Recalculates layout when viewport height changes (e.g., window resize).
	 * Maintains sticky bottom behavior if enabled.
	 */
	$effect(() => {
		if (viewportHeight && previousViewportHeight !== viewportHeight) {
			untrack(async () => {
				const nearBottom = wasNearBottom();
				await recalculateVisibleRange();
				if (stickToBottom && nearBottom) {
					scrollToBottom();
				}
				previousViewportHeight = viewportHeight;
			});
		}
	});

	$inspect(visibleRange);
	$inspect(visible);
	let containerHeight = $state(0);
	$inspect({
		containerHeight,
		scrollHeight: viewport?.scrollHeight,
		scrollTop: viewport?.scrollTop
	});
	/**
	 * Handles new items being added to the list.
	 * Auto-scrolls if sticky bottom is enabled, otherwise shows notification.
	 */
	$effect(() => {
		if (items && viewport) {
			untrack(async () => {
				log('items changed', {
					previousDistance,
					distance: getDistanceFromBottom()
				});
				heightMap.length = items.length;
				// First-time initialization for tail mode
				if (!isInitialized() && items.length > 0) {
					const index = stickToBottom ? items.length - 1 : startIndex || 0;
					await initializeAt(index);
					if (!isInitialized()) {
						return;
					}
				}

				if (stickToBottom) {
					log('stick to bottom', {
						previousDistance,
						distance: getDistanceFromBottom(),
						nearBottom: wasNearBottom()
					});
					if (wasNearBottom()) {
						await recalculateVisibleRange();
						if (getDistanceFromBottom() !== 0) {
							log('path 8');
							scrollToBottom();
						}
					} else {
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

	export function log(...args: any[]): void {
		// eslint-disable-next-line no-console
		console.log(`[${Date.now().toString().slice(-5)}]`, ...args);
	}
</script>

<ScrollableContainer
	bind:viewportHeight
	bind:viewport
	zIndex="3"
	onscroll={() => {
		if (!viewport) return;
		if (ignoreScroll) {
			ignoreScroll = false;
			return;
		}
		const scrollTop = viewport?.scrollTop;
		if (lastScrollTop && lastScrollTop > scrollTop) {
			log('up scroll detected', scrollTop, lastScrollTop);
			lastScrollDirection = 'up';
		} else if (lastScrollTop && lastScrollTop < scrollTop) {
			log('down scroll detected', scrollTop, lastScrollTop);
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
		bind:this={container}
		bind:clientHeight={containerHeight}
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
				console.log('blah', { previousDistance, distance: getDistanceFromBottom() });
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
