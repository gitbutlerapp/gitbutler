<script lang="ts">
	import VirtualList from "$components/VirtualList.svelte";
	import AsyncContent from "$lib/helpers/AsyncContent.svelte";

	type Props = {
		defaultHeight: number;
		itemCount?: number;
		stickToBottom?: boolean;
		asyncContent?: { delay: number; height: number };
		startIndex?: number;
		onloadmore?: () => Promise<void>;
		showBottomButton?: boolean;
		onVisibleChange?: (change: { start: number; end: number } | undefined) => void;
		renderDistance?: number;
		/**
		 * When set, onloadmore will automatically prepend this many items.
		 * Simulates the IRC history-loading pattern where scrolling near the top
		 * loads older messages that get prepended to the list.
		 */
		loadMorePrependCount?: number;
		/**
		 * When true, items get variable heights based on index:
		 * - Every 5th item: 200px (like a message with code block)
		 * - Every 3rd item: 60px (medium message)
		 * - Others: 30px (short message)
		 * This overrides the default .test-item min-height: 100px.
		 */
		variableHeights?: boolean;
		/**
		 * When set, overrides the default 100px min-height on .test-item.
		 * Use to simulate small items like IRC rows (e.g., itemHeight: 16).
		 */
		itemHeight?: number;
	};

	const {
		defaultHeight,
		asyncContent,
		itemCount = 10,
		stickToBottom = false,
		startIndex,
		onloadmore,
		showBottomButton = false,
		onVisibleChange,
		renderDistance = 0,
		loadMorePrependCount,
		variableHeights = false,
		itemHeight,
	}: Props = $props();

	function getItemHeight(index: number): number {
		if (index % 5 === 0) return 200; // code block / long message
		if (index % 3 === 0) return 60; // medium message
		return 30; // short message
	}

	let items = $state(Array.from({ length: itemCount }, (_, i) => `Item ${i + 1}`));
	let container = $state<HTMLDivElement>();
	let loadMoreCallCount = $state(0);
	let virtualList = $state<VirtualList<any>>();
	let showFooter = $state(false);
	let visibleStart = $state(-1);
	let visibleEnd = $state(-1);

	function handleVisibleChange(change: { start: number; end: number } | undefined) {
		visibleStart = change?.start ?? -1;
		visibleEnd = change?.end ?? -1;
		onVisibleChange?.(change);
	}

	// Wrap the onloadmore prop to track call count and optionally auto-prepend
	async function handleLoadMore() {
		loadMoreCallCount++;
		if (loadMorePrependCount) {
			const count = loadMorePrependCount;
			const base = items.length;
			const newItems = Array.from({ length: count }, (_, i) => `History ${base + i + 1}`);
			items.unshift(...newItems);
		}
		if (onloadmore) {
			await onloadmore();
		}
	}

	// Expose jumpToIndex method
	export async function jumpToIndex(index: number) {
		if (virtualList?.jumpToIndex) {
			await virtualList.jumpToIndex(index);
		}
	}

	// Expose scrollToBottom method
	export function scrollToBottom() {
		if (virtualList?.scrollToBottom) {
			virtualList.scrollToBottom();
		}
	}

	function addItem() {
		items.push(`Item ${items.length + 1}`);
	}

	function prependItem() {
		items.unshift(`Prepended ${Date.now()}`);
	}

	function replaceAllItems() {
		items = Array.from({ length: 15 }, (_, i) => `Replaced ${i + 1}`);
	}

	/**
	 * Simulates switching to a completely different view.
	 * Both head and tail IDs change, and the item count differs.
	 */
	let chatId = $state(0);
	function switchItems() {
		chatId++;
		const count = chatId % 2 === 1 ? 74 : 101;
		items = Array.from({ length: count }, (_, i) => `Chat${chatId}-Msg${i}`);
	}

	/**
	 * Prepend a batch of items (simulates history loading).
	 * Adds 50 items at the front with unique IDs.
	 */
	let batchId = $state(0);
	function prependBatch() {
		batchId++;
		const newItems = Array.from({ length: 50 }, (_, i) => `Batch${batchId}-${i}`);
		items = [...newItems, ...items];
	}

	function removeLastItem() {
		if (items.length > 0) {
			items.pop();
		}
	}

	function addBatchItems() {
		const start = items.length;
		for (let i = 0; i < 10; i++) {
			items.push(`Batch ${start + i + 1}`);
		}
	}

	function expandLast() {
		// Need to wait for next tick to ensure DOM is updated
		requestAnimationFrame(() => {
			const itemElements = document.querySelectorAll(".test-item");
			if (itemElements && itemElements.length > 0) {
				const lastItem = itemElements[itemElements.length - 1] as HTMLElement;
				lastItem.style.height = "300px";
			}
		});
	}

	let jumpToIndexValue = $state(0);

	function handleJumpToIndex() {
		if (virtualList?.jumpToIndex) {
			virtualList.jumpToIndex(jumpToIndexValue);
		}
	}

	function handleScrollToBottom() {
		if (virtualList?.scrollToBottom) {
			virtualList.scrollToBottom();
		}
	}

	function handleScrollToTop() {
		if (virtualList) {
			virtualList.jumpToIndex(0);
		}
	}

	function toggleFooter() {
		showFooter = !showFooter;
	}

	function toggleFooterAndAddItem() {
		showFooter = !showFooter;
		items.push(`Item ${items.length + 1}`);
	}
</script>

<div
	bind:this={container}
	class="test-container"
	data-testid="test-container"
	data-loadmore-count={loadMoreCallCount}
>
	<VirtualList
		bind:this={virtualList}
		{items}
		{stickToBottom}
		{defaultHeight}
		{startIndex}
		{showBottomButton}
		{renderDistance}
		onloadmore={onloadmore || loadMorePrependCount ? handleLoadMore : undefined}
		onVisibleChange={handleVisibleChange}
		visibility="hover"
		getId={(item) => item}
	>
		{#snippet template(item, index)}
			<div
				class="test-item"
				style:min-height={itemHeight
					? `${itemHeight}px`
					: variableHeights
						? `${getItemHeight(index)}px`
						: undefined}
			>
				<p>{item}</p>
				{#if asyncContent}
					{@const { delay, height } = asyncContent}
					<AsyncContent {delay}>
						<p class="async-content" style:height>async content</p>
					</AsyncContent>
				{/if}
			</div>
		{/snippet}
		{#if showFooter}
			<div class="footer" data-testid="footer">Footer Content</div>
		{/if}
	</VirtualList>
	<span data-testid="visible-start" style="display:none">{visibleStart}</span>
	<span data-testid="visible-end" style="display:none">{visibleEnd}</span>
	<div class="controls">
		<button type="button" onclick={addItem}>Add Item</button>
		<button type="button" onclick={prependItem} data-testid="prepend-item-button">
			Prepend Item
		</button>
		<button type="button" onclick={replaceAllItems} data-testid="replace-items-button">
			Replace Items
		</button>
		<button type="button" onclick={removeLastItem} data-testid="remove-last-button">
			Remove Last
		</button>
		<button type="button" onclick={addBatchItems} data-testid="add-batch-button">
			Add Batch
		</button>
		<button type="button" onclick={expandLast}>Expand Last</button>
		<button type="button" onclick={switchItems} data-testid="switch-chat-button">
			Switch Chat
		</button>
		<button type="button" onclick={prependBatch} data-testid="prepend-batch-button">
			Prepend Batch
		</button>
		<button type="button" onclick={toggleFooter} data-testid="toggle-footer-button">
			Toggle Footer
		</button>
		<button
			type="button"
			onclick={toggleFooterAndAddItem}
			data-testid="toggle-footer-and-add-item-button"
		>
			Toggle Footer + Add
		</button>
		<input
			type="number"
			bind:value={jumpToIndexValue}
			data-testid="jump-to-index-input"
			style="width: 60px; padding: 4px;"
		/>
		<button type="button" onclick={handleJumpToIndex} data-testid="jump-to-index-button">
			Jump To Index
		</button>
		<button type="button" onclick={handleScrollToBottom} data-testid="scroll-to-bottom-button">
			Scroll To Bottom
		</button>
		<button type="button" onclick={handleScrollToTop} data-testid="scroll-to-top-button">
			Scroll To Top
		</button>
	</div>
</div>

<style>
	.test-container {
		display: flex;
		flex-direction: column;
		width: 400px;
		height: 400px;
	}

	.test-item {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 100px;
		gap: 12px;
		border: 1px solid #ccc;
		background: white;
	}

	.async-content {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 200px;
		padding: 12px;
		border: 1px solid lightgrey;
	}

	.controls {
		display: flex;
		padding: 8px;
		gap: 8px;
		background: #f0f0f0;
	}

	button {
		padding: 8px 16px;
		border: none;
		border-radius: 4px;
		background: #007bff;
		color: white;
		cursor: pointer;
	}

	button:hover {
		background: #0056b3;
	}

	.footer {
		display: flex;
		align-items: center;
		justify-content: center;
		height: 200px;
		padding: 12px;
		border: 2px solid blue;
		background: lightblue;
		font-weight: bold;
	}
</style>
