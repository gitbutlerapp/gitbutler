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

<script lang="ts" generics="T">
	import Button from '$components/Button.svelte';
	import ScrollableContainer from '$components/scroll/ScrollableContainer.svelte';

	import { debounce } from '$lib/utils/debounce';

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
	const STICKY_DISTANCE = 200;
	const LOAD_MORE_THRESHOLD = 300;

	// Debounce load more callback
	const debouncedLoadMore = $derived(debounce(() => onloadmore?.(), 50));

	// DOM references
	let viewport = $state<HTMLDivElement>();
	let visibleRowElements = $state<HTMLCollectionOf<Element>>(); // This is a live list
	let resizeObserver: ResizeObserver | null = null;
	let viewportHeight = $state(0);
	let previousViewportHeight = 0;

	// Virtual scrolling state
	let visibleRange = $state({
		start: tail ? Infinity : 0,
		end: tail ? Infinity : 0
	});

	// An array mapping items to element heights
	let heightMap: Array<number | undefined> = $state([]);

	// Padding that takes up out of viewport space
	let offset = $state({ top: 0, bottom: 0 });

	let totalHeight = $state(0);

	// Chat-specific state
	let lastDistanceFromBottom = $state(0);
	let hasInitialized = $state(false);
	let wasAtBottomBeforeResize = $state(false);
	let previousItemsLength = $state(items.length);
	let hasNewUnreadItems = $state(false);

	const itemChunks = $derived(divideIntoChunks(items, batchSize));
	const visibleChunks = $derived.by(() =>
		itemChunks
			.slice(visibleRange.start, visibleRange.end)
			.map((data, i) => ({ id: i + visibleRange.start, data }))
	);

	function divideIntoChunks<T>(array: T[], size: number) {
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

	function saveDistanceFromBottom() {
		if (viewport) {
			lastDistanceFromBottom = getDistanceFromBottom();
		}
	}

	function isScrollNearBottom() {
		return getDistanceFromBottom() < STICKY_DISTANCE;
	}

	function shouldTriggerLoadMore() {
		return totalHeight < viewportHeight || getDistanceFromBottom() < LOAD_MORE_THRESHOLD;
	}

	function getDistanceFromBottom() {
		if (!viewport) return 0;
		return viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
	}

	async function measureRowHeight(i: number): Promise<number> {
		if (i < visibleRange.start) {
			return heightMap[i] || defaultHeight;
		}

		let rowElement = visibleRowElements?.[i - visibleRange.start];
		if (!rowElement) {
			await tick(); // render the newly visible row
			rowElement = visibleRowElements?.[i - visibleRange.start];
			if (!rowElement) return defaultHeight;
		}
		const rowHeight = (rowElement as HTMLElement)?.offsetHeight || defaultHeight;
		heightMap[i] = rowHeight;
		return rowHeight;
	}

	async function calculateVisibleStartIndex(): Promise<number> {
		if (itemChunks.length === 0) {
			return 0;
		}

		let accumulatedHeight = 0;
		let i = 0;

		while (i < itemChunks.length) {
			const rowHeight = await measureRowHeight(i);
			accumulatedHeight += rowHeight;

			if (accumulatedHeight > viewport!.scrollTop) {
				return i;
			}
			i++;
		}
		return itemChunks.length - 1;
	}

	async function calculateVisibleEndIndex(): Promise<number> {
		let accumulatedHeight = offset.top - viewport!.scrollTop;
		let i = visibleRange.start;

		while (i < itemChunks.length) {
			if (!visibleRowElements![i - visibleRange.start]) {
				visibleRange.end = i + 1;
				offset.bottom = calculateHeightSum(visibleRange.end, heightMap.length);
				await tick(); // render the newly visible row
			}
			const rowHeight = await measureRowHeight(i);

			accumulatedHeight += rowHeight;
			if (accumulatedHeight > viewport!.clientHeight) {
				return i + 1;
			}
			i++;
		}
		return itemChunks.length;
	}

	async function calculateStartIndexFromBottom(): Promise<number> {
		if (!viewport) return 0;

		let accumulatedHeight = 0;
		let i = visibleRange.end - 1;

		while (i >= 0) {
			// Set startIndex to render this chunk
			visibleRange.start = i;
			await tick(); // Wait for the chunk to render

			// Now measure the actual rendered height
			const rowElement = visibleRowElements?.[0]; // First row in the visible set -- this is not safe
			const rowHeight = (rowElement as HTMLElement)?.offsetHeight || defaultHeight;
			heightMap[i] = rowHeight;

			accumulatedHeight += rowHeight;

			if (accumulatedHeight > viewport.clientHeight) {
				return i;
			}
			i--;
		}
		return 0;
	}

	let isRecalculating = false;

	async function recalculateVisibleRange() {
		if (!viewport || !visibleRowElements) return;
		if (isRecalculating) return; // One at a time.

		isRecalculating = true;
		heightMap.length = itemChunks.length;

		// Handle bottom initialization
		if (!hasInitialized && tail) {
			// Start from the last chunk and work backwards
			visibleRange.end = itemChunks.length;
			offset.bottom = 0;
			await tick();

			visibleRange.start = await calculateStartIndexFromBottom();
			offset.top = calculateHeightSum(0, visibleRange.start);
			totalHeight = calculateHeightSum(0, heightMap.length);

			setTimeout(() => {
				if (viewport) {
					viewport.scrollTop = viewport.scrollHeight;
					hasInitialized = true;
				}
			}, 20);
		} else {
			await tick();
			const previousStartIndex = visibleRange.start;

			visibleRange = {
				start: await calculateVisibleStartIndex(),
				end: await calculateVisibleEndIndex()
			};
			offset = {
				bottom: calculateHeightSum(visibleRange.end, heightMap.length),
				top: calculateHeightSum(0, visibleRange.start)
			};

			if (visibleRange.start < previousStartIndex) {
				await tick();
				const cachedHeight = heightMap[visibleRange.start] || defaultHeight;
				const realHeight = (visibleRowElements[0] as HTMLElement)?.offsetHeight;
				const heightDifference = realHeight - cachedHeight;
				if (heightDifference !== 0) {
					viewport.scrollBy({ top: heightDifference });
				}
			}
			await tick();
			totalHeight = calculateHeightSum(0, heightMap.length);
		}

		if (shouldTriggerLoadMore()) {
			debouncedLoadMore?.();
		}

		for (const rowElement of visibleRowElements) {
			resizeObserver?.observe(rowElement);
		}

		saveDistanceFromBottom();
		isRecalculating = false;
	}

	// Setup resize observer when viewport is ready
	$effect(() => {
		if (viewport) {
			visibleRowElements = viewport.getElementsByClassName('list-row');
			resizeObserver = new ResizeObserver(() =>
				untrack(() => {
					// recalculateVisibleRange();
					const hasGrown = getDistanceFromBottom() > lastDistanceFromBottom;
					if (hasGrown && stickToBottom && lastDistanceFromBottom < STICKY_DISTANCE) {
						if (viewport) {
							viewport.scrollTo({
								top: viewport.scrollHeight,
								behavior: hasInitialized ? 'smooth' : 'instant'
							});
						}
					}
				})
			);
			return () => {
				resizeObserver?.disconnect();
			};
		}
	});

	// Recalculate when viewport height changes
	$effect(() => {
		if (viewportHeight && previousViewportHeight !== viewportHeight) {
			// Track if we were at bottom before resize
			if (stickToBottom && viewport) {
				wasAtBottomBeforeResize = isScrollNearBottom();
			}

			untrack(async () => {
				await recalculateVisibleRange();
				// Restore bottom position if we were at bottom and stickToBottom is enabled
				if (stickToBottom && wasAtBottomBeforeResize && viewport) {
					viewport.scrollTop = viewport.scrollHeight;
					await tick();
				}
				previousViewportHeight = viewportHeight;
			});
		}
	});

	async function scrollToBottomAndDismissNotification() {
		if (!viewport) return;
		hasNewUnreadItems = false;
		visibleRange = { end: itemChunks.length, start: itemChunks.length - 1 };
		offset = { bottom: 0, top: calculateHeightSum(0, visibleRange.start) };
		lastDistanceFromBottom = 0;
		await tick();
		viewport.scrollTo({ top: viewport.scrollHeight, behavior: 'instant' });
	}

	// Auto-scroll to bottom when new items are added (if stickToBottom is enabled)
	$effect(() => {
		if (items && stickToBottom && isScrollNearBottom()) {
			if (!viewport) return;
			untrack(async () => {
				await recalculateVisibleRange();
				// It appears we need to wait for the next animation frame in order
				// for the new element to have the correct dimensions. Without this
				// delay it often happens we scroll past the text, but not to the
				// bottom of the chat bubble.
				setTimeout(() => {
					if (!viewport) return;
					viewport.scrollTo({
						top: viewport.scrollHeight,
						behavior: hasInitialized ? 'smooth' : 'instant'
					});
				}, 0);
			});
		} else if (items) {
			untrack(() => {
				const hadNewItems = items.length > previousItemsLength && items.length > visibleRange.end;
				recalculateVisibleRange();
				if (tail && hadNewItems) {
					hasNewUnreadItems = true;
				}
			});
		}
		previousItemsLength = items.length;
	});
</script>

<ScrollableContainer
	bind:viewportHeight
	bind:viewport
	onscroll={() => recalculateVisibleRange()}
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
		{#each visibleChunks as chunk (chunk.id)}
			<!-- Note: keying this #each would make things much slower. -->
			<div class="list-row">
				{@render chunkTemplate?.(chunk.data)}
			</div>
		{/each}
		{@render children?.()}
	</div>
</ScrollableContainer>
{#if lastDistanceFromBottom > 600}
	<div class="feed-actions">
		{#if hasNewUnreadItems}
			<button
				type="button"
				class="text-12 feed-actions__new-messages"
				transition:fade={{ duration: 150 }}
				onclick={scrollToBottomAndDismissNotification}
			>
				New unread
			</button>
		{/if}
		<div class="feed-actions__scroll-to-bottom" transition:fade={{ duration: 150 }}>
			<Button
				kind="outline"
				icon="arrow-down"
				tooltip="Scroll to bottom"
				onclick={scrollToBottomAndDismissNotification}
			/>
		</div>
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
		padding: 0 8px;
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
