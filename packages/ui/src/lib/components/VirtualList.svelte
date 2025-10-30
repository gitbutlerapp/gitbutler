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
		/** Whether to initialize scroll position at top or bottom. */
		initialPosition?: 'top' | 'bottom';
		/** Auto-scroll to bottom when new items are added (useful for chat). */
		stickToBottom?: boolean;
		visibility: ScrollbarVisilitySettings;
		padding?: {
			left?: number;
			right?: number;
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
		initialPosition = 'top',
		stickToBottom = false
	}: Props = $props();

	// Constants
	const STICKY_DISTANCE = 100;
	const FALLBACK_HEIGHT = 65;
	const LOAD_MORE_THRESHOLD = 150;

	// Debounce load more callback
	const debouncedLoad = $derived(debounce(() => onloadmore?.(), 100));

	// DOM references
	let viewport = $state<HTMLDivElement>();
	let rows = $state<HTMLCollectionOf<Element>>(); // This is a live list
	let resizeObserver: ResizeObserver | null = null;
	let viewportHeight = $state(0);
	let previousViewportHeight = 0;

	// Virtual scrolling state
	let start = $state(initialPosition === 'bottom' ? Infinity : 0);
	let end = $state(initialPosition === 'bottom' ? Infinity : 0);

	// An array mapping items to element heights
	let heightMap: Array<number | undefined> = $state([]);

	// Padding that takes up out of viewport space
	let topPadding = $state(0);
	let bottomPadding = $state(0);

	let totalHeight = $state(0);

	// Chat-specific state
	let isNearBottom = $state(true);
	let distanceFromBottom = $state(0);
	let hasInitialized = $state(false);
	let wasAtBottomBeforeResize = $state(false);
	let previousItemsLength = $state(items.length);
	let newUnseenTail = $state(false);

	const chunks = $derived(chunk(items, batchSize));
	const visible = $derived.by(() =>
		chunks.slice(start, end).map((data, i) => ({ id: i + start, data }))
	);

	function chunk<T>(arr: T[], size: number) {
		return Array.from({ length: Math.ceil(arr.length / size) }, (_v, i) =>
			arr.slice(i * size, i * size + size)
		);
	}

	function sumHeights(startIndex: number, endIndex: number): number {
		let sum = 0;
		for (let i = startIndex; i < endIndex; i++) {
			sum += heightMap[i] || FALLBACK_HEIGHT;
		}
		return sum;
	}

	function checkIfNearBottom() {
		if (!viewport) return;
		distanceFromBottom = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
		isNearBottom = distanceFromBottom < STICKY_DISTANCE;
		return isNearBottom;
	}

	async function getRowHeight(i: number, rowOffset: number): Promise<number> {
		if (i < start) {
			return heightMap[i] || FALLBACK_HEIGHT;
		}

		let rowElement = rows?.[rowOffset];
		if (!rowElement) {
			await tick(); // render the newly visible row
			rowElement = rows?.[rowOffset];
			if (!rowElement) return FALLBACK_HEIGHT;
		}
		const rowHeight = (rowElement as HTMLElement)?.offsetHeight || FALLBACK_HEIGHT;
		heightMap[i] = rowHeight;
		return rowHeight;
	}

	async function updateStartIndex(): Promise<number> {
		let accumulatedHeight = 0;
		let oldStart = start;
		let i = 0;

		while (i < chunks.length) {
			const rowHeight = await getRowHeight(i, oldStart - i);
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
		let i = start;

		while (i < chunks.length) {
			if (!rows![i - start]) {
				end = i + 1;
				bottomPadding = sumHeights(end, heightMap.length);
				await tick(); // render the newly visible row
			}
			const rowHeight = await getRowHeight(i, i - start);

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
			start = i;
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

	async function recalculate(isScroll?: boolean) {
		if (!viewport || !rows) return;

		heightMap.length = chunks.length;

		// Handle bottom initialization
		if (!hasInitialized && initialPosition === 'bottom') {
			// Start from the last chunk and work backwards
			end = chunks.length;
			bottomPadding = 0;
			await tick();

			start = await updateStartIndexBackwards();
			topPadding = sumHeights(0, start);
			totalHeight = sumHeights(0, heightMap.length);

			setTimeout(() => {
				if (viewport) {
					viewport.scrollTop = viewport.scrollHeight;
					hasInitialized = true;
				}
			}, 20);
		} else {
			// Normal top-down calculation
			// There is some weird bug here that seems triggered when
			// start has moved forward, as an element is tagen out from
			// the top but equally compensated by new padding, the
			// scrollTop suddenly jumps by a lot.
			let scrollTop = viewport.scrollTop;

			await tick();

			const savedDistance = distanceFromBottom;
			const oldStart = start;
			const newStart = await updateStartIndex();
			topPadding = sumHeights(0, newStart);
			await tick();

			start = newStart;

			if (start < oldStart) {
				await tick();
				const cachedHeight = heightMap[start] || FALLBACK_HEIGHT;
				const realHeight = (rows[0] as HTMLElement).offsetHeight;
				const diff = realHeight - cachedHeight;
				if (diff !== 0) {
					viewport.scrollBy({ top: diff });
					scrollTop += diff;
				}
			}
			await tick();
			// Resetting the scroll top here seems to give us the correct behavior.
			if (viewport.scrollTop !== scrollTop) {
				viewport.scrollTop = scrollTop;
				await tick();
			}

			end = await updateEndIndex();
			bottomPadding = sumHeights(end, heightMap.length);

			await tick();

			if (
				!isScroll &&
				stickToBottom &&
				savedDistance < STICKY_DISTANCE &&
				distanceFromBottom > savedDistance
			) {
				viewport.scrollTop = viewport.scrollHeight;
				await tick();
			}

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
					await tick();
				}
				previousViewportHeight = viewportHeight;
			});
		}
	});

	function scrollToBottomAndDismiss() {
		if (viewport) {
			newUnseenTail = false;
			viewport.scrollTo({ top: viewport.scrollHeight, behavior: 'instant' });
		}
	}

	// Auto-scroll to bottom when new items are added (if stickToBottom is enabled)
	$effect(() => {
		if (items && stickToBottom && isNearBottom) {
			if (!viewport) return;
			untrack(async () => {
				await recalculate();
				// It appears we need to wait for the next animation frame in order
				// for the new element to have the correct dimensions. Without this
				// delay it often happens we scroll past the text, but not to the
				// bottom of the chat bubble.
				requestAnimationFrame(() => {
					if (!viewport) return;
					viewport.scrollTo({ top: viewport.scrollHeight, behavior: 'smooth' });
				});
			});
		} else if (items) {
			untrack(() => {
				const hadNewItems = items.length > previousItemsLength && items.length > end;
				recalculate();
				if (hadNewItems) {
					newUnseenTail = true;
				}
			});
		}
		previousItemsLength = items.length;
	});
</script>

<ScrollableContainer
	bind:viewportHeight
	bind:viewport
	onscroll={() => {
		recalculate(true);
		checkIfNearBottom();
	}}
	wide={grow}
	whenToShow={visibility}
	{padding}
>
	<div
		data-remove-from-panning
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
		{@render children?.()}
	</div>
</ScrollableContainer>

{#if distanceFromBottom > 300}
	<div class="feed-actions">
		{#if newUnseenTail}
			<button
				type="button"
				class="text-12 feed-actions__new-messages"
				transition:fade={{ duration: 150 }}
				onclick={scrollToBottomAndDismiss}
			>
				New unread
			</button>
		{/if}
		<div class="feed-actions__scroll-to-bottom" transition:fade={{ duration: 150 }}>
			<Button
				kind="outline"
				icon="arrow-down"
				tooltip="Scroll to bottom"
				onclick={scrollToBottomAndDismiss}
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
