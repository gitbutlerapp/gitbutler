<!--
	VirtualList - A high-performance virtual scrolling component

	This component renders large lists efficiently by only rendering items that are
	currently visible in the viewport. It:
	- Chunks items into batches for optimized rendering
	- Dynamically measures row heights and caches them
	- Uses padding to maintain scroll position for off-screen items
	- Supports infinite scrolling with the onloadmore callback
	- Automatically recalculates layout on resize and item changes
-->
<script lang="ts" module>
	type T = unknown;
</script>

<script lang="ts" generics="T = any">
	import Button from '$components/Button.svelte';
	import ScrollableContainer from '$components/scroll/ScrollableContainer.svelte';

	import { debounce } from '$lib/utils/debounce';

	import { resizeObserver } from '$lib/utils/resizeObserver';
	import { tick, untrack, type Snippet } from 'svelte';
	import { fade } from 'svelte/transition';
	import type { ScrollbarVisilitySettings } from '$components/scroll/Scrollbar.svelte';

	type Props = {
		items: Array<T>;
		/** Items that are always included. */
		children?: Snippet<[]>;
		/** Template for group of items. */
		chunkTemplate: Snippet<[T[]]>;
		/** Number of items grouped together. */
		batchSize: number;
		/** Handler for when scroll has reached with a margin of the bottom. */
		onloadmore?: () => Promise<void>;
		grow?: boolean;
		/** Whether to initialize scroll position at top or bottom (tail). */
		tail?: boolean;
		/** Auto-scroll to bottom when new items are added (useful for chat). */
		stickToBottom?: boolean;
		visibility: ScrollbarVisilitySettings;
		/** Default height of chunked container element. */
		defaultHeight: number;
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
		tail,
		stickToBottom = false
	}: Props = $props();

	// Constants
	const SCROLL_DOWN_THRESHOLD = 600;
	const STICKY_DISTANCE = 40;
	const LOAD_MORE_THRESHOLD = 200;
	const DEBOUNCE_DELAY = 50;

	// Debounce load more callback
	const debouncedLoadMore = $derived(debounce(() => onloadmore?.(), DEBOUNCE_DELAY));

	// DOM references
	let viewport = $state<HTMLDivElement>();
	let visibleRowElements = $state<HTMLCollectionOf<Element>>();
	let itemObserver: ResizeObserver | null = null;
	let viewportHeight = $state(0);
	let previousViewportHeight = 0;

	// Virtual scrolling state
	let visibleRange = $state({
		start: tail ? Infinity : 0,
		end: tail ? Infinity : 0
	});
	let heightMap: number[] = $state([]);
	let offset = $state({ top: 0, bottom: 0 });
	let totalHeight = $state(0);
	let previousDistance = $state(0);
	let isInitialized = $state(false);
	let previousCount = $state(items.length);
	let hasNewItemsAtBottom = $state(false);
	let isRecalculating = false;

	// Derived state
	const itemChunks = $derived(divideIntoChunks(items, batchSize));
	const visibleChunks = $derived.by(() =>
		itemChunks
			.slice(visibleRange.start, visibleRange.end)
			.map((data, i) => ({ id: i + visibleRange.start, data }))
	);

	// Helper functions
	function divideIntoChunks<T>(array: T[], size: number): T[][] {
		return Array.from({ length: Math.ceil(array.length / size) }, (_v, i) =>
			array.slice(i * size, i * size + size)
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
		return totalHeight < viewportHeight || getDistanceFromBottom() < LOAD_MORE_THRESHOLD;
	}

	function isNearBottom(): boolean {
		return previousDistance < STICKY_DISTANCE;
	}

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

	function initializeRange(): void {
		visibleRange.end = itemChunks.length;
		offset.bottom = 0;

		visibleRange.start = calculateStartIndexFromBottom();
		offset.top = calculateHeightSum(0, visibleRange.start);
		totalHeight = calculateHeightSum(0, heightMap.length);

		// Delay scroll to ensure DOM is ready
		setTimeout(() => {
			scrollToBottom();
			isInitialized = true;
		}, 0);
	}

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

	async function recalculateVisibleRange(): Promise<void> {
		if (!viewport || !visibleRowElements || isRecalculating) return;

		isRecalculating = true;
		heightMap.length = itemChunks.length;

		const scrollTop = viewport.scrollTop;

		if (!isInitialized && tail) {
			initializeRange();
		} else {
			updateRange();
		}

		await tick();
		totalHeight = calculateHeightSum(0, heightMap.length);
		viewport.scrollTop = scrollTop;

		if (shouldTriggerLoadMore()) {
			debouncedLoadMore?.();
		}

		// Observe all visible row elements for size changes
		for (const rowElement of visibleRowElements) {
			itemObserver?.observe(rowElement);
		}

		saveDistanceFromBottom();
		isRecalculating = false;
	}

	// Setup resize observer when viewport is ready
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
						if (tail && index === visibleRange.start) {
							const heightDiff = target.clientHeight - defaultHeight;
							if (heightDiff !== 0) {
								viewport?.scrollBy({ top: heightDiff });
							}
						}
						// Even if not sticky, if we are starting at the end we need
						// to scroll to bottom when it resizes.
						if ((stickToBottom || (!isInitialized && tail)) && index === visibleRange.end - 1) {
							scrollToBottom();
						}
					}
				}

				// Auto-scroll if we're near the bottom and content grew
				if (viewport && stickToBottom && isNearBottom()) {
					const hasGrown = getDistanceFromBottom() > previousDistance;
					if (hasGrown) {
						scrollToBottom();
					}
				}
			})
		);

		return () => itemObserver?.disconnect();
	});

	// Recalculate when viewport height changes
	$effect(() => {
		if (viewportHeight && previousViewportHeight !== viewportHeight) {
			untrack(async () => {
				await recalculateVisibleRange();
				if (stickToBottom && isNearBottom()) {
					scrollToBottom();
				}
				previousViewportHeight = viewportHeight;
			});
		}
	});

	export function scrollToBottom(): void {
		if (!viewport) return;
		viewport.scrollTop = viewport.scrollHeight - viewport.clientHeight;
	}

	// Handle new items being added
	$effect(() => {
		if (items) {
			untrack(async () => {
				if (stickToBottom && isNearBottom()) {
					// User is at the bottom, auto-scroll
					await recalculateVisibleRange();
					scrollToBottom();
				} else {
					// Check if there are new items at the bottom
					if (tail) {
						const count = items.length;
						hasNewItemsAtBottom = count > previousCount && count > visibleRange.end;
					}
					recalculateVisibleRange();
				}
			});
		}
		previousCount = items.length;
	});

	// Clear "new items" indicator when user scrolls to bottom
	$effect(() => {
		if (hasNewItemsAtBottom && visibleRange.end === itemChunks.length) {
			hasNewItemsAtBottom = false;
		}
	});
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
				{@render chunkTemplate?.(chunk.data)}
			</div>
		{/each}
		<div
			class="children"
			use:resizeObserver={() => {
				if (isNearBottom()) {
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
