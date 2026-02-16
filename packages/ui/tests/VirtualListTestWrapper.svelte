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
	};

	const {
		defaultHeight,
		asyncContent,
		itemCount = 10,
		stickToBottom = false,
		startIndex,
		onloadmore,
	}: Props = $props();

	let items = $state(Array.from({ length: itemCount }, (_, i) => `Item ${i + 1}`));
	let container = $state<HTMLDivElement>();
	let loadMoreCallCount = $state(0);
	let virtualList = $state<any>();
	let showFooter = $state(false);

	// Wrap the onloadmore prop to track call count
	async function handleLoadMore() {
		loadMoreCallCount++;
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

	function toggleFooter() {
		showFooter = !showFooter;
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
		onloadmore={onloadmore ? handleLoadMore : undefined}
		visibility="hover"
	>
		{#snippet template(item)}
			<div class="test-item">
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
	<div class="controls">
		<button type="button" onclick={addItem}>Add Item</button>
		<button type="button" onclick={expandLast}>Expand Last</button>
		<button type="button" onclick={toggleFooter} data-testid="toggle-footer-button">
			Toggle Footer
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
