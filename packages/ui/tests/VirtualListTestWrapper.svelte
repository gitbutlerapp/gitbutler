<script lang="ts">
	import VirtualList from '$components/VirtualList.svelte';

	type Props = {
		batchSize: number;
		itemCount?: number;
		stickToBottom?: boolean;
		tail?: boolean;
	};

	const { itemCount = 10, stickToBottom = false, tail, batchSize }: Props = $props();

	let items = $state(Array.from({ length: itemCount }, (_, i) => `Item ${i + 1}`));
	let container = $state<HTMLDivElement>();

	function addItems() {
		items.push(`Item ${items.length + 1}`);
	}

	function expandLast() {
		// Need to wait for next tick to ensure DOM is updated
		requestAnimationFrame(() => {
			const itemElements = document.querySelectorAll('.test-item');
			if (itemElements && itemElements.length > 0) {
				const lastItem = itemElements[itemElements.length - 1] as HTMLElement;
				lastItem.style.height = '300px';
			}
		});
	}
</script>

<div bind:this={container} class="test-container">
	<VirtualList {items} {stickToBottom} {tail} {batchSize} defaultHeight={150} visibility="hover">
		{#snippet chunkTemplate(chunk)}
			{#each chunk as item}
				<div class="test-item">
					{item}
				</div>
			{/each}
		{/snippet}
	</VirtualList>
	<div class="controls">
		<button type="button" onclick={addItems}>Add Items</button>
		<button type="button" onclick={expandLast}>Expand Last</button>
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
		align-items: center;
		justify-content: center;
		height: 100px;
		border: 1px solid #ccc;
		background: white;
		transition: height 0.3s;
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
</style>
