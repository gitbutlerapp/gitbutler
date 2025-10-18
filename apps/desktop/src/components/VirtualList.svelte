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
	import { SETTINGS } from '$lib/settings/userSettings';
	import { chunk } from '$lib/utils/array';
	import { debounce } from '$lib/utils/debounce';
	import { inject } from '@gitbutler/core/context';
	import { ScrollableContainer } from '@gitbutler/ui';

	import { tick, untrack, type Snippet } from 'svelte';

	type Props = {
		items: Array<T>;
		/** Template for group of items. */
		chunkTemplate: Snippet<[T[]]>;
		/** Number of items grouped together. */
		batchSize: number;
		/** Handler for when scroll has reached with a margin of the bottom. */
		onloadmore?: () => Promise<void>;
		start?: number;
		grow?: boolean;
		/** Whether to initialize scroll position at top or bottom. */
		initialPosition?: 'top' | 'bottom';
		/** Auto-scroll to bottom when new items are added (useful for chat). */
		stickToBottom?: boolean;
		padding?: {
			left?: number;
			right?: number;
		};
	};

	const {
		items,
		chunkTemplate,
		batchSize,
		onloadmore,
		grow,
		padding,
		start,
		initialPosition = 'top',
		stickToBottom = false
	}: Props = $props();

	const userSettings = inject(SETTINGS);

	// Constants
	const FALLBACK_HEIGHT = 40;
	const LOAD_MORE_THRESHOLD = 150;

	// Debounce load more callback
	const debouncedLoad = $derived(debounce(() => onloadmore?.(), 100));

	// DOM references
	let viewport = $state<HTMLDivElement>();
	let rows = $state<HTMLCollectionOf<Element>>(); // This is a live list
	let resizeObserver: ResizeObserver | null = null;
	let viewportHeight = $state(0);

	// Virtual scrolling state
	let startIndex = $state(start ?? (initialPosition === 'bottom' ? Infinity : 0));
	let end = $state(start ?? (initialPosition === 'bottom' ? Infinity : 0));

	// An array mapping items to element heights
	let heightMap: Array<number | undefined> = $state([]);

	// Padding that takes up out of viewport space
	let topPadding = $state(0);
	let bottomPadding = $state(0);

	let totalHeight = $state(0);

	// Chat-specific state
	let isNearBottom = $state(true);
	let hasInitialized = $state(false);
	let previousItemsLength = $state(items.length);
	let previousViewportHeight = $state(viewportHeight);
	let wasAtBottomBeforeResize = $state(false);

	const chunks = $derived(chunk(items, batchSize));
	const visible = $derived.by(() =>
		chunks.slice(startIndex, end).map((data, i) => ({ id: i + startIndex, data }))
	);

	function sumHeights(startIndex: number, endIndex: number): number {
		let sum = 0;
		for (let i = startIndex; i < endIndex; i++) {
			sum += heightMap[i] || FALLBACK_HEIGHT;
		}
		return sum;
	}

	function checkIfNearBottom() {
		if (!viewport) return;
		const threshold = 50;
		const distanceFromBottom = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
		isNearBottom = distanceFromBottom < threshold;
	}

	async function getRowHeight(i: number, rowOffset: number): Promise<number> {
		if (i < startIndex) {
			return heightMap[i] || FALLBACK_HEIGHT;
		}

		let rowElement = rows![rowOffset];
		if (!rowElement) {
			await tick(); // render the newly visible row
			rowElement = rows![i < startIndex ? i : rowOffset];
		}
		const rowHeight = (rowElement as HTMLElement)?.offsetHeight || FALLBACK_HEIGHT;
		heightMap[i] = rowHeight;
		return rowHeight;
	}

	async function updateStartIndex(): Promise<number> {
		let accumulatedHeight = 0;
		let oldStart = startIndex;
		let i = 0;

		while (i < chunks.length) {
			const rowHeight = await getRowHeight(i, i - oldStart);
			accumulatedHeight += rowHeight;

			if (accumulatedHeight > viewport!.scrollTop) {
				return i;
			}
			i += 1;
		}
		return i;
	}

	async function updateEndIndex(): Promise<number> {
		let accumulatedHeight = topPadding - viewport!.scrollTop;
		let i = startIndex;

		while (i < chunks.length) {
			if (!rows![i - startIndex]) {
				end = i + 1;
				bottomPadding = sumHeights(end, heightMap.length);
				await tick(); // render the newly visible row
			}
			const rowHeight = await getRowHeight(i, i - startIndex);

			accumulatedHeight += rowHeight;
			if (accumulatedHeight > viewport!.clientHeight) {
				return i + 1;
			}
			i += 1;
		}
		return i;
	}

	async function updateStartIndexBackwards(): Promise<number> {
		if (!viewport) return 0;

		let accumulatedHeight = 0;
		let i = end - 1;

		while (i >= 0) {
			// Set startIndex to render this chunk
			startIndex = i;
			await tick(); // Wait for the chunk to render

			// Now measure the actual rendered height
			const rowElement = rows![0]; // First row in the visible set
			const rowHeight = (rowElement as HTMLElement)?.offsetHeight || FALLBACK_HEIGHT;
			heightMap[i] = rowHeight;

			accumulatedHeight += rowHeight;

			if (accumulatedHeight > viewport.clientHeight) {
				return i;
			}
			i -= 1;
		}
		return 0;
	}

	async function recalculate() {
		if (!viewport || !rows) return;

		heightMap.length = chunks.length;

		// Handle bottom initialization
		if (!hasInitialized && initialPosition === 'bottom') {
			// Start from the last chunk and work backwards
			end = chunks.length;
			bottomPadding = 0;
			await tick();

			startIndex = await updateStartIndexBackwards();
			topPadding = sumHeights(0, startIndex);

			totalHeight = sumHeights(0, heightMap.length);

			// Scroll to bottom
			viewport.scrollTop = viewport.scrollHeight;
			hasInitialized = true;
		} else {
			// Normal top-down calculation
			await tick();
			startIndex = await updateStartIndex();
			topPadding = sumHeights(0, startIndex);

			// Calculate visible range end
			await tick();
			end = await updateEndIndex();
			bottomPadding = sumHeights(end, heightMap.length);

			totalHeight = sumHeights(0, heightMap.length);
		}

		// Trigger load more if needed
		const shouldLoad =
			totalHeight < viewportHeight ||
			viewport.scrollTop + viewportHeight > totalHeight - LOAD_MORE_THRESHOLD;
		if (shouldLoad) {
			debouncedLoad?.();
		}

		// Observe all visible rows for resize
		for (const rowElement of rows) {
			resizeObserver?.observe(rowElement);
		}
	}

	// Setup resize observer when viewport is ready
	$effect(() => {
		if (viewport) {
			rows = viewport.getElementsByClassName('list-row');
			resizeObserver = new ResizeObserver(() => untrack(() => recalculate()));
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
				wasAtBottomBeforeResize = isNearBottom;
			}

			untrack(async () => {
				await recalculate();
				// Restore bottom position if we were at bottom and stickToBottom is enabled
				if (stickToBottom && wasAtBottomBeforeResize && viewport) {
					viewport.scrollTop = viewport.scrollHeight;
				}
				previousViewportHeight = viewportHeight;
			});
		}
	});

	// Recalculate when items change
	$effect(() => {
		if (items) {
			untrack(() => recalculate());
		}
	});

	// Note: Bottom initialization is now handled in recalculate()

	// Auto-scroll to bottom when new items are added (if stickToBottom is enabled)
	$effect(() => {
		if (items.length > previousItemsLength && stickToBottom && isNearBottom) {
			untrack(() => {
				tick().then(() => {
					if (viewport) {
						viewport.scrollTop = viewport.scrollHeight;
					}
				});
			});
		}
		previousItemsLength = items.length;
	});
</script>

<ScrollableContainer
	bind:viewportHeight
	bind:viewport
	whenToShow={$userSettings.scrollbarVisibilityState}
	onscroll={() => {
		recalculate();
		checkIfNearBottom();
	}}
	wide={grow}
	{padding}
>
	<div
		class="padded-contents"
		style:padding-top={topPadding + 'px'}
		style:padding-bottom={bottomPadding + 'px'}
	>
		{#each visible as chunk}
			<!-- Note: keying this #each would make things much slower. -->
			<div class="list-row">
				{@render chunkTemplate?.(chunk.data)}
			</div>
		{/each}
	</div>
</ScrollableContainer>

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
</style>
