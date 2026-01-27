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
		renderDistance?: number;
		/**
		 * Whether to show the "Scroll to bottom" button when user scrolls up.
		 * Defaults to false.
		 */
		showBottomButton?: boolean;
		/**
		 * Callback for when the visible items change. Note that this is not necessarily the
		 * same as the rendered range when `renderDistance !== 0`.
		 * @param change
		 */
		onVisibleChange?: (change: { start: number; end: number }) => void;
	};

	const SCROLL_DOWN_THRESHOLD = 600;
	const STICKY_DISTANCE = 40;
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
		onVisibleChange
	}: Props = $props();

	let viewport = $state<HTMLDivElement>();
	let container = $state<HTMLDivElement>();
	let visibleRowElements = $state<HTMLCollectionOf<Element>>();

	let heightMap: number[] = $state([]);
	let lockedHeights = $state<number[]>([]);
	let heightUnlockTimeouts: number[] = [];
	let viewportHeight = $state(0);
	let previousViewportHeight = 0;

	/* Item index range that are rendered. */
	let renderRange = $state({ start: 0, end: 0 });
	/* Item index range that are visible in the viewport. */
	let visibleRange = $state({ start: 0, end: 0 });

	let offset = $state({ top: 0, bottom: 0 });
	let previousDistance = $state(0);
	let previousCount = $state(items.length);
	let hasNewItemsAtBottom = $state(false);
	let isRecalculating = false;

	let lastScrollTop: number | undefined = undefined;
	let lastScrollDirection: 'up' | 'down' | undefined;
	let lastJumpToIndex: number | undefined;
	let skipNextScrollEvent = false;

	const debouncedLoadMore = $derived(debounce(() => onloadmore?.(), DEBOUNCE_DELAY));

	const visible = $derived.by(() =>
		items
			.slice(renderRange.start, renderRange.end)
			.map((data, i) => ({ id: i + renderRange.start, data }))
	);

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
				const oldHeight = heightMap[index] || defaultHeight;
				if (target.clientHeight !== heightMap[index]) {
					heightMap[index] = target.clientHeight;
				}

				if (
					lastScrollDirection === 'up' &&
					calculateHeightSum(0, renderRange.start) !== viewport.scrollTop &&
					visibleRange.start === index
				) {
					// When scrolling up, compensate for the height change of the topmost
					// visible element to prevent content from jumping downward.
					viewport.scrollBy({ top: heightMap[index] - oldHeight });
				} else if (
					(lastJumpToIndex !== undefined || startIndex) &&
					lastScrollDirection === undefined
				) {
					// After jumping to an index, maintain position as off-viewport elements
					// resize. Scroll direction is undefined during jumps.
					viewport.scrollTop = calculateHeightSum(0, lastJumpToIndex || startIndex || 0);
					skipNextScrollEvent = true;
				} else if (stickToBottom && previousDistance < STICKY_DISTANCE) {
					// Maintain bottom position when near bottom and `stickToBottom` is true
					skipNextScrollEvent = true;
					scrollToBottom();
				} else if (
					previousDistance < STICKY_DISTANCE &&
					index === items.length - 1 &&
					viewport.scrollTop !== 0
				) {
					// When we are not at the scrollTop, but near the bottom, if the last
					// element resizes we scroll to bottom
					skipNextScrollEvent = true;
					scrollToBottom();
				}
			}
		}

		if (shouldRecalculate) {
			recalculateRanges();
		}
	});

	const mutationObserver = new MutationObserver((mutations) => {
		for (const mutation of mutations) {
			for (const node of mutation.addedNodes) {
				if (node instanceof HTMLElement && node.matches('[data-index]')) {
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

	function isInitialized() {
		return renderRange.end !== 0;
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

	/**
	 * Locks an element to its previous height such that it has time to resume
	 * its proper height.
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
	 * Adjusts the top and bottom padding based on the height map and visible indices.
	 */
	function updateOffsets() {
		offset = {
			top: calculateHeightSum(0, renderRange.start),
			bottom: calculateHeightSum(renderRange.end, heightMap.length)
		};
	}

	/**
	 * Returns newly vsisible indices based on old and new ranges. This is
	 * primarily used for locking elements coming into view to their
	 * previously measured heights.
	 */
	function getNewIndices(oldStart: number, oldEnd: number, newStart: number, newEnd: number) {
		const result: number[] = [];
		for (let i = newStart; i < Math.min(newEnd, oldStart); i++) result.push(i);
		for (let i = Math.max(newStart, oldEnd); i < newEnd; i++) result.push(i);
		return result;
	}

	function calculateStartIndex(): [number, number] {
		if (items.length === 0 || !viewport) return [0, 0];

		let renderStart = 0;
		let accumulatedHeight = 0;

		for (let i = 0; i < items.length; i++) {
			const rowHeight = visibleRowElements?.[i - renderRange.start]?.clientHeight;
			const heightToUse = rowHeight || heightMap[i] || defaultHeight;
			accumulatedHeight += heightToUse;
			if (accumulatedHeight >= viewport.scrollTop - renderDistance) {
				renderStart = i;
			}
			if (accumulatedHeight >= viewport.scrollTop) {
				return [renderStart, i];
			}
		}
		return [items.length - 1, items.length - 1];
	}

	/**
	 * Important: This index is not inclusive.
	 */
	function calculateEndIndex(): [number, number] {
		const count = items.length;
		if (!viewport) return [count, count];

		let visibleEnd = 0;
		let accumulatedHeight = offset.top - viewport.scrollTop;
		for (let i = renderRange.start; i < count; i++) {
			accumulatedHeight += heightMap[i] || defaultHeight;
			if (accumulatedHeight > viewport.clientHeight) {
				visibleEnd = i + 1;
			}
			if (accumulatedHeight > viewport.clientHeight + renderDistance) {
				return [i + 1, visibleEnd];
			}
		}
		return [count, count];
	}

	async function initializeAt(startingAt: number): Promise<void> {
		if (!viewport) return;

		// Initialize from start position downwards
		renderRange.start = startingAt;
		for (let i = startingAt; i < items.length; i++) {
			if (heightMap[i] !== undefined) {
				lockRowHeight(i);
			}
			renderRange.end = i + 1;
			await tick(); // Wait for element to be added
			const element = visibleRowElements?.[i - startingAt];
			if (!element) {
				// Most likely cause for this is that something else made `visibleRange.end = 0`
				// during the tick(). This needs debugging, but is not severe enough to warrant
				// immediate attention.
				console.warn('Invariant violation - root cause not yet determined.');
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

		// Continues upwards if there is space.
		for (let i = startingAt - 1; i >= 0; i--) {
			if (heightMap[i] !== undefined) {
				lockRowHeight(i);
			}
			renderRange.start = i;
			await tick(); // Wait for element to be added
			const element = visibleRowElements?.[0];
			if (!element) {
				console.warn('Invariant violation - root cause not yet determined.');
				return;
			}
			const heightDiff = element.clientHeight - (lockedHeights[i] || defaultHeight);
			if (heightDiff !== 0) {
				viewport.scrollTop += heightDiff;
			}
			heightMap[i] = element.clientHeight;
			if (calculateHeightSum(renderRange.start, startingAt) > renderDistance) {
				break;
			}
		}

		if (!isInitialized()) {
			return;
		}

		updateOffsets();
		await tick();

		// Based on anecdotal evidence the type for `viewport` seems incorrect. It's likely
		// that during some kind of unmount event it can become `undefined` due to a subtle
		// reactivity condition.
		if (viewport) viewport.scrollTop = calculateHeightSum(0, startingAt);
	}

	async function recalculateRanges(): Promise<void> {
		if (!viewport || !visibleRowElements || !isInitialized() || isRecalculating) return;

		isRecalculating = true;

		const oldStart = renderRange.start;
		const oldEnd = renderRange.end;
		const bottomDistance = getDistanceFromBottom();

		const [start, visibleStart] = calculateStartIndex();
		const [end, visibleEnd] = calculateEndIndex();

		if (start !== renderRange.start || end !== renderRange.end) {
			renderRange = { start, end };
			updateOffsets();

			const newIndices = getNewIndices(oldStart, oldEnd, renderRange.start, renderRange.end);
			for (const index of newIndices) {
				if (heightMap[index]) {
					lockRowHeight(index);
				}
			}

			await tick();
		}

		if (visibleStart !== visibleRange.start || visibleEnd !== visibleRange.end) {
			visibleRange = { start: visibleStart, end: visibleEnd };
		}

		if (stickToBottom && bottomDistance === 0 && bottomDistance !== getDistanceFromBottom()) {
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
		viewport.scrollTo({
			top: viewport.scrollHeight - viewport.clientHeight,
			behavior: 'instant'
		});
	}

	export async function jumpToIndex(index: number) {
		// Based on anecdotal evidence the type for `items` seems incorrect. It's likely
		// that during some kind of unmount event it can become `undefined` due to
		// some subtle reactivity condition.
		if (!items || index < 0 || index > items.length - 1) {
			return;
		}
		lastScrollDirection = undefined;
		skipNextScrollEvent = true;
		lastJumpToIndex = index;
		lockRowHeight(index);
		initializeAt(index);
	}

	$effect(() => {
		if (container) {
			mutationObserver.observe(container, { childList: true });
			for (const el of container.querySelectorAll(':scope > [data-index]')) {
				itemObserver.observe(el);
			}
		}
		return () => {
			mutationObserver.disconnect();
			itemObserver.disconnect();
		};
	});

	$effect(() => {
		if (!viewport) return;
		visibleRowElements = viewport.getElementsByClassName('list-row');
	});

	$effect(() => {
		if (viewportHeight && viewportHeight < previousViewportHeight) {
			untrack(async () => {
				const nearBottom = previousDistance < STICKY_DISTANCE;
				await recalculateRanges();
				if (stickToBottom && nearBottom) {
					scrollToBottom();
				}
				previousViewportHeight = viewportHeight;
			});
		}
	});

	$effect(() => {
		if (items && viewport) {
			untrack(async () => {
				heightMap.length = items.length;
				if (!isInitialized() && items.length > 0) {
					const index = stickToBottom ? items.length - 1 : startIndex || 0;
					await initializeAt(index);
					if (!isInitialized()) {
						return;
					}
				}

				if (stickToBottom) {
					if (previousDistance < STICKY_DISTANCE) {
						await recalculateRanges();
						if (getDistanceFromBottom() !== 0) {
							scrollToBottom();
						}
					} else {
						const count = items.length;
						hasNewItemsAtBottom = count > previousCount && count > visibleRange.end;
					}
				}
				updateOffsets();
			});
		}
		previousCount = items.length;
	});

	$effect(() => {
		if (hasNewItemsAtBottom && renderRange.end === items.length) {
			hasNewItemsAtBottom = false;
		}
	});

	$effect(() => {
		if (visibleRange.end !== 0) {
			onVisibleChange?.(visibleRange);
		}
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
		const scrollTop = viewport?.scrollTop;
		if (lastScrollTop && lastScrollTop > scrollTop) {
			lastScrollDirection = 'up';
		} else if (lastScrollTop && lastScrollTop < scrollTop) {
			lastScrollDirection = 'down';
		} else {
			lastScrollDirection = undefined;
		}
		recalculateRanges();
		lastScrollTop = viewport.scrollTop;
	}}
	wide={grow}
	whenToShow={visibility}
	{padding}
>
	<div
		bind:this={container}
		data-remove-from-panning
		class="padded-contents"
		style:padding-top={offset.top + 'px'}
		style:padding-bottom={offset.bottom + 'px'}
	>
		{#each visible as item, i (item.id)}
			<div
				class="list-row"
				data-index={i + renderRange.start}
				style:height={lockedHeights[i + renderRange.start]
					? `${lockedHeights[i + renderRange.start]}px`
					: undefined}
			>
				{@render template(item.data, renderRange.start + i)}
			</div>
		{/each}
		<div
			class="children"
			use:resizeObserver={() => {
				if (stickToBottom && previousDistance < STICKY_DISTANCE) {
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
		{#if showBottomButton && previousDistance > SCROLL_DOWN_THRESHOLD}
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
